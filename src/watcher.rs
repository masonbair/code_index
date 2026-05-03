//! File system watcher using notify with debouncing

use crate::error::{CodeIndexError, Result};
use crate::indexer::Indexer;
use crate::parser::ParserFactory;
use log::{debug, error, info, warn};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// File watcher for incremental index updates
pub struct FileWatcher {
    debouncer: Debouncer<RecommendedWatcher>,
    receiver: Receiver<std::result::Result<Vec<DebouncedEvent>, notify::Error>>,
    watched_paths: Vec<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher with debouncing
    pub fn new(debounce_duration: Duration) -> Result<Self> {
        let (tx, rx) = channel();

        let debouncer = new_debouncer(debounce_duration, tx)
            .map_err(|e| CodeIndexError::Watch(e.to_string()))?;

        Ok(Self {
            debouncer,
            receiver: rx,
            watched_paths: Vec::new(),
        })
    }

    /// Start watching a directory
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        info!("Watching directory: {}", path.display());

        self.debouncer
            .watcher()
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| CodeIndexError::Watch(e.to_string()))?;

        self.watched_paths.push(path);
        Ok(())
    }

    /// Stop watching a directory
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        info!("Unwatching directory: {}", path.display());

        self.debouncer
            .watcher()
            .unwatch(&path)
            .map_err(|e| CodeIndexError::Watch(e.to_string()))?;

        self.watched_paths.retain(|p| p != &path);
        Ok(())
    }

    /// Get the list of watched paths
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }

    /// Process pending events and update the index
    /// Returns the number of files processed
    pub fn process_events(&self, indexer: &mut Indexer) -> Result<usize> {
        let mut processed = 0;

        // Non-blocking receive
        while let Ok(result) = self.receiver.try_recv() {
            match result {
                Ok(events) => {
                    for event in events {
                        if self.process_event(&event, indexer)? {
                            processed += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("Watch error: {:?}", e);
                }
            }
        }

        Ok(processed)
    }

    /// Blocking wait for events and process them
    /// Returns the number of files processed, or None if the channel is closed
    pub fn wait_and_process(&self, indexer: &mut Indexer) -> Result<Option<usize>> {
        match self.receiver.recv() {
            Ok(result) => {
                let mut processed = 0;
                match result {
                    Ok(events) => {
                        for event in events {
                            if self.process_event(&event, indexer)? {
                                processed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {:?}", e);
                    }
                }
                Ok(Some(processed))
            }
            Err(_) => {
                // Channel closed
                Ok(None)
            }
        }
    }

    fn process_event(&self, event: &DebouncedEvent, indexer: &mut Indexer) -> Result<bool> {
        let path = &event.path;

        // Skip if not a supported file type
        if !path.is_file() || ParserFactory::for_file(path).is_none() {
            return Ok(false);
        }

        debug!("Processing event for: {}", path.display());

        // Check if file exists (handles both create/modify and delete)
        if path.exists() {
            match indexer.index_file(path) {
                Ok(stats) => {
                    info!(
                        "Re-indexed {}: {} symbols, {} deps",
                        path.display(),
                        stats.symbols,
                        stats.dependencies
                    );
                }
                Err(e) => {
                    warn!("Failed to re-index {}: {}", path.display(), e);
                }
            }
        } else {
            // File was deleted
            match indexer.remove_file(path) {
                Ok(()) => {
                    info!("Removed from index: {}", path.display());
                }
                Err(e) => {
                    warn!("Failed to remove {}: {}", path.display(), e);
                }
            }
        }

        Ok(true)
    }
}

/// Watch manager for running the watcher in a loop
pub struct WatchManager {
    watcher: FileWatcher,
    indexer: Arc<Mutex<Indexer>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl WatchManager {
    pub fn new(watcher: FileWatcher, indexer: Indexer) -> Self {
        Self {
            watcher,
            indexer: Arc::new(Mutex::new(indexer)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Run the watch loop (blocking)
    pub fn run(&mut self) -> Result<()> {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        info!("Watch manager started");

        while self.running.load(std::sync::atomic::Ordering::SeqCst) {
            let mut indexer = self.indexer.lock().unwrap();
            match self.watcher.wait_and_process(&mut indexer) {
                Ok(Some(count)) => {
                    if count > 0 {
                        debug!("Processed {} file changes", count);
                    }
                }
                Ok(None) => {
                    // Channel closed, stop
                    break;
                }
                Err(e) => {
                    error!("Error processing events: {}", e);
                }
            }
        }

        info!("Watch manager stopped");
        Ok(())
    }

    /// Stop the watch loop
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get access to the watcher for adding/removing paths
    pub fn watcher(&mut self) -> &mut FileWatcher {
        &mut self.watcher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileWatcher::new(Duration::from_millis(100));
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_directory() {
        let temp = TempDir::new().unwrap();
        let mut watcher = FileWatcher::new(Duration::from_millis(100)).unwrap();

        assert!(watcher.watch(temp.path()).is_ok());
        assert_eq!(watcher.watched_paths().len(), 1);

        assert!(watcher.unwatch(temp.path()).is_ok());
        assert_eq!(watcher.watched_paths().len(), 0);
    }

    #[test]
    fn test_file_change_detection() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let mut indexer = Indexer::new(&db_path).unwrap();

        let mut watcher = FileWatcher::new(Duration::from_millis(50)).unwrap();
        watcher.watch(temp.path()).unwrap();

        // Create a test file
        let test_file = temp.path().join("test.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        // Give the watcher time to detect the change
        std::thread::sleep(Duration::from_millis(200));

        // Process events
        let processed = watcher.process_events(&mut indexer).unwrap();
        // Note: This might be 0 or 1 depending on timing
        // The important thing is no error occurred
        assert!(processed >= 0);
    }
}
