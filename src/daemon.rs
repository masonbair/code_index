//! Daemon management for background file watching

use crate::error::{CodeIndexError, Result};
use crate::indexer::Indexer;
use crate::watcher::{FileWatcher, WatchManager};
use daemonize::Daemonize;
use log::{error, info};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

/// Default paths for daemon files
pub fn default_pid_path() -> PathBuf {
    dirs_cache().join("code-index.pid")
}

pub fn default_stdout_path() -> PathBuf {
    dirs_cache().join("code-index.out")
}

pub fn default_stderr_path() -> PathBuf {
    dirs_cache().join("code-index.err")
}

pub fn default_db_path() -> PathBuf {
    dirs_cache().join("code-index.db")
}

fn dirs_cache() -> PathBuf {
    // Use XDG_CACHE_HOME or ~/.cache
    if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(cache).join("ai-tools")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".cache").join("ai-tools")
    } else {
        PathBuf::from("/tmp/ai-tools")
    }
}

/// Daemon status
#[derive(Debug, Clone)]
pub enum DaemonStatus {
    Running {
        pid: u32,
        watched_dirs: Vec<PathBuf>,
    },
    Stopped,
    Error(String),
}

impl std::fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonStatus::Running { pid, watched_dirs } => {
                write!(f, "Running (PID: {})", pid)?;
                if !watched_dirs.is_empty() {
                    write!(f, "\nWatched directories:")?;
                    for dir in watched_dirs {
                        write!(f, "\n  - {}", dir.display())?;
                    }
                }
                Ok(())
            }
            DaemonStatus::Stopped => write!(f, "Stopped"),
            DaemonStatus::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// Daemon manager
pub struct DaemonManager {
    pid_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    db_path: PathBuf,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            pid_path: default_pid_path(),
            stdout_path: default_stdout_path(),
            stderr_path: default_stderr_path(),
            db_path: default_db_path(),
        }
    }

    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.db_path = path;
        self
    }

    /// Get the current daemon status
    pub fn status(&self) -> DaemonStatus {
        match self.read_pid() {
            Some(pid) => {
                if self.is_process_running(pid) {
                    DaemonStatus::Running {
                        pid,
                        watched_dirs: self.read_watched_dirs().unwrap_or_default(),
                    }
                } else {
                    // Stale PID file
                    let _ = fs::remove_file(&self.pid_path);
                    DaemonStatus::Stopped
                }
            }
            None => DaemonStatus::Stopped,
        }
    }

    /// Start the daemon
    pub fn start(&self, watch_paths: &[PathBuf], foreground: bool) -> Result<()> {
        // Check if already running
        if let DaemonStatus::Running { pid, .. } = self.status() {
            return Err(CodeIndexError::Daemon(format!(
                "Daemon already running with PID {}",
                pid
            )));
        }

        // Ensure directories exist
        if let Some(parent) = self.pid_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if foreground {
            self.run_foreground(watch_paths)
        } else {
            self.run_daemon(watch_paths)
        }
    }

    /// Stop the daemon
    pub fn stop(&self) -> Result<()> {
        match self.read_pid() {
            Some(pid) => {
                info!("Stopping daemon (PID: {})", pid);

                // Send SIGTERM
                unsafe {
                    if libc::kill(pid as i32, libc::SIGTERM) != 0 {
                        return Err(CodeIndexError::Daemon(format!(
                            "Failed to send signal to PID {}",
                            pid
                        )));
                    }
                }

                // Wait for process to exit (with timeout)
                for _ in 0..50 {
                    if !self.is_process_running(pid) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }

                // Clean up PID file
                let _ = fs::remove_file(&self.pid_path);

                info!("Daemon stopped");
                Ok(())
            }
            None => Err(CodeIndexError::Daemon("Daemon is not running".to_string())),
        }
    }

    /// Restart the daemon
    pub fn restart(&self, watch_paths: &[PathBuf], foreground: bool) -> Result<()> {
        if matches!(self.status(), DaemonStatus::Running { .. }) {
            self.stop()?;
            // Small delay to ensure clean shutdown
            std::thread::sleep(Duration::from_millis(500));
        }
        self.start(watch_paths, foreground)
    }

    fn run_foreground(&self, watch_paths: &[PathBuf]) -> Result<()> {
        info!("Starting daemon in foreground mode");

        // Write PID file
        let pid = std::process::id();
        self.write_pid(pid)?;

        // Set up signal handler for clean shutdown
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            info!("Received shutdown signal");
            r.store(false, std::sync::atomic::Ordering::SeqCst);
        })
        .map_err(|e| CodeIndexError::Daemon(e.to_string()))?;

        // Run the watch loop
        let result = self.run_watch_loop(watch_paths, running);

        // Clean up
        let _ = fs::remove_file(&self.pid_path);

        result
    }

    fn run_daemon(&self, watch_paths: &[PathBuf]) -> Result<()> {
        info!("Starting daemon in background mode");

        let stdout = File::create(&self.stdout_path)?;
        let stderr = File::create(&self.stderr_path)?;

        let daemonize = Daemonize::new()
            .pid_file(&self.pid_path)
            .chown_pid_file(true)
            .working_directory("/tmp")
            .stdout(stdout)
            .stderr(stderr);

        // Clone paths for the child process
        let paths = watch_paths.to_vec();
        let db_path = self.db_path.clone();

        match daemonize.start() {
            Ok(()) => {
                // We're in the child process now
                env_logger::init();

                let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

                // Set up signal handler
                unsafe {
                    libc::signal(libc::SIGTERM, handle_sigterm as *const () as usize);
                }

                // Store the running flag globally for the signal handler
                RUNNING.with(|cell| {
                    *cell.borrow_mut() = Some(running.clone());
                });

                // Initialize and run
                let indexer = match Indexer::new(&db_path) {
                    Ok(i) => i,
                    Err(e) => {
                        error!("Failed to create indexer: {}", e);
                        std::process::exit(1);
                    }
                };

                let mut watcher = match FileWatcher::new(Duration::from_secs(2)) {
                    Ok(w) => w,
                    Err(e) => {
                        error!("Failed to create watcher: {}", e);
                        std::process::exit(1);
                    }
                };

                for path in &paths {
                    if let Err(e) = watcher.watch(path) {
                        error!("Failed to watch {}: {}", path.display(), e);
                    }
                }

                let mut manager = WatchManager::new(watcher, indexer);

                if let Err(e) = manager.run() {
                    error!("Watch manager error: {}", e);
                    std::process::exit(1);
                }

                std::process::exit(0);
            }
            Err(e) => Err(CodeIndexError::Daemon(format!(
                "Failed to daemonize: {}",
                e
            ))),
        }
    }

    fn run_watch_loop(
        &self,
        watch_paths: &[PathBuf],
        running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        let indexer = Indexer::new(&self.db_path)?;

        let mut watcher = FileWatcher::new(Duration::from_secs(2))?;

        for path in watch_paths {
            watcher.watch(path)?;
        }

        // Save watched directories for status command
        self.write_watched_dirs(watcher.watched_paths())?;

        let manager = WatchManager::new(watcher, indexer);

        // Run until stopped
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            // Process events with a timeout
            std::thread::sleep(Duration::from_millis(100));
        }

        manager.stop();
        Ok(())
    }

    fn read_pid(&self) -> Option<u32> {
        fs::read_to_string(&self.pid_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    fn write_pid(&self, pid: u32) -> Result<()> {
        if let Some(parent) = self.pid_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&self.pid_path)?;
        write!(file, "{}", pid)?;
        Ok(())
    }

    fn is_process_running(&self, pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    fn read_watched_dirs(&self) -> Result<Vec<PathBuf>> {
        let path = dirs_cache().join("code-index-watched.txt");
        match fs::read_to_string(&path) {
            Ok(content) => Ok(content.lines().map(PathBuf::from).collect()),
            Err(_) => Ok(Vec::new()),
        }
    }

    fn write_watched_dirs(&self, dirs: &[PathBuf]) -> Result<()> {
        let path = dirs_cache().join("code-index-watched.txt");
        let content: String = dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, content)?;
        Ok(())
    }
}

impl Default for DaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local storage for the running flag (for signal handler)
thread_local! {
    static RUNNING: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> =
        std::cell::RefCell::new(None);
}

extern "C" fn handle_sigterm(_: i32) {
    RUNNING.with(|cell| {
        if let Some(running) = cell.borrow().as_ref() {
            running.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_status_display() {
        let status = DaemonStatus::Stopped;
        assert_eq!(format!("{}", status), "Stopped");

        let status = DaemonStatus::Running {
            pid: 1234,
            watched_dirs: vec![PathBuf::from("/home/user/project")],
        };
        let display = format!("{}", status);
        assert!(display.contains("1234"));
        assert!(display.contains("/home/user/project"));
    }

    #[test]
    fn test_daemon_manager_status_when_stopped() {
        let manager = DaemonManager::new();
        // Should be stopped when no PID file exists
        assert!(matches!(manager.status(), DaemonStatus::Stopped));
    }

    #[test]
    fn test_default_paths() {
        let pid = default_pid_path();
        let db = default_db_path();

        assert!(pid.to_string_lossy().contains("code-index.pid"));
        assert!(db.to_string_lossy().contains("code-index.db"));
    }
}
