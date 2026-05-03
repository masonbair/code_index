//! code-index CLI: Persistent semantic cache for AI agents

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use code_index::{
    daemon::{default_db_path, DaemonManager, DaemonStatus},
    indexer::{Database, DirectoryIndexStats, Indexer, SymbolKind, UsageKind},
    query::{
        CallResult, CalleesResult, CallersResult, DependencyQueryResult, DependencyResult,
        FileResult, HotFilesResult, ImplementationResult, ImplementorsResult, LanguageStat,
        QueryEngine, StatsResult, SymbolQueryResult, SymbolResult, TraitsResult, UnusedResult,
        UsageResult, UsagesQueryResult,
    },
    OutputFormat,
};
use log::{info, LevelFilter};
use std::path::PathBuf;

/// Persistent semantic cache for AI agents
#[derive(Parser)]
#[command(name = "code-index")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the database file
    #[arg(long, global = true)]
    db_path: Option<PathBuf>,

    /// Verbose output (can be repeated for more verbosity)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Quiet mode (suppress non-error output)
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Output format
    #[arg(long, global = true, default_value = "human")]
    format: OutputFormatArg,

    /// Output as JSON (shortcut for --format=json)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormatArg {
    Human,
    Json,
    Compact,
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Human => OutputFormat::Human,
            OutputFormatArg::Json => OutputFormat::Json,
            OutputFormatArg::Compact => OutputFormat::Compact,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Daemon management commands
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Index a directory (one-time, no watching)
    Index {
        /// Path to index
        path: PathBuf,
    },

    /// Re-index from scratch
    Reindex {
        /// Path to re-index (default: current directory)
        #[arg(long)]
        path: Option<PathBuf>,

        /// Re-index a specific file only
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Query the index
    Query {
        #[command(subcommand)]
        query_type: QueryType,
    },

    /// Show index statistics
    Stats,

    /// Clear the entire index
    Clear {
        /// Confirm the clear operation
        #[arg(long)]
        confirm: bool,
    },

    /// Export index to JSON
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon
    Start {
        /// Directory to watch (can be specified multiple times)
        #[arg(long)]
        watch: Vec<PathBuf>,

        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },

    /// Stop the daemon
    Stop,

    /// Restart the daemon
    Restart {
        /// Directory to watch
        #[arg(long)]
        watch: Vec<PathBuf>,

        /// Run in foreground
        #[arg(long)]
        foreground: bool,
    },

    /// Show daemon status
    Status,
}

#[derive(Subcommand)]
enum QueryType {
    /// Find symbols by name
    Symbol {
        /// Symbol name to search for
        name: String,

        /// Use pattern matching (substring search)
        #[arg(long)]
        pattern: bool,
    },

    /// Get all symbols in a file
    File {
        /// File path
        path: PathBuf,
    },

    /// Get dependencies of a file
    Dependencies {
        /// File path
        path: PathBuf,

        /// Show files that depend on this file (reverse lookup)
        #[arg(long)]
        reverse: bool,
    },

    /// Get all dependencies in the index
    AllDependencies {
        /// Maximum number of dependencies to return
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Get hot files (frequently changed/complex)
    HotFiles {
        /// Number of files to return
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// List symbols by kind
    Kind {
        /// Symbol kind (function, class, struct, interface, type, enum, etc.)
        kind: String,

        /// Maximum number of results
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Find all callers of a function/method
    Callers {
        /// Name of the function/method to find callers for
        name: String,
    },

    /// Find all functions/methods called by a function
    Callees {
        /// Name of the function/method
        name: String,
    },

    /// Find all types that implement a trait
    Implements {
        /// Trait name
        trait_name: String,
    },

    /// Find all traits implemented by a type
    Traits {
        /// Type name
        type_name: String,
    },

    /// Find all usages of a symbol
    Usages {
        /// Symbol name to find usages for
        name: String,

        /// Filter by usage kind (type_annotation, call, field_access, etc.)
        #[arg(long)]
        kind: Option<String>,

        /// Limit to a specific file
        #[arg(long, name = "in")]
        in_file: Option<PathBuf>,
    },

    /// Find potentially unused symbols
    Unused {
        /// Limit to a specific file
        #[arg(long, name = "in")]
        in_file: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging
    let log_level = if cli.quiet {
        LevelFilter::Error
    } else {
        match cli.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp(None)
        .format_target(false)
        .init();

    // Determine output format
    let output_format = if cli.json {
        OutputFormat::Json
    } else {
        cli.format.into()
    };

    // Get database path
    let db_path = cli.db_path.unwrap_or_else(default_db_path);

    match cli.command {
        Commands::Daemon { action } => handle_daemon(action, &db_path),
        Commands::Index { path } => handle_index(&path, &db_path, output_format),
        Commands::Reindex { path, file } => handle_reindex(path, file, &db_path, output_format),
        Commands::Query { query_type } => handle_query(query_type, &db_path, output_format),
        Commands::Stats => handle_stats(&db_path, output_format),
        Commands::Clear { confirm } => handle_clear(confirm, &db_path),
        Commands::Export { output } => handle_export(&output, &db_path),
    }
}

fn handle_daemon(action: DaemonAction, db_path: &PathBuf) -> Result<()> {
    let manager = DaemonManager::new().with_db_path(db_path.clone());

    match action {
        DaemonAction::Start { watch, foreground } => {
            let paths = if watch.is_empty() {
                vec![std::env::current_dir()?]
            } else {
                watch
            };

            manager
                .start(&paths, foreground)
                .context("Failed to start daemon")?;

            if !foreground {
                println!("Daemon started");
            }
        }
        DaemonAction::Stop => {
            manager.stop().context("Failed to stop daemon")?;
            println!("Daemon stopped");
        }
        DaemonAction::Restart { watch, foreground } => {
            let paths = if watch.is_empty() {
                vec![std::env::current_dir()?]
            } else {
                watch
            };

            manager
                .restart(&paths, foreground)
                .context("Failed to restart daemon")?;

            if !foreground {
                println!("Daemon restarted");
            }
        }
        DaemonAction::Status => {
            let status = manager.status();
            match status {
                DaemonStatus::Running { pid, watched_dirs } => {
                    println!("Status: Running");
                    println!("PID: {}", pid);
                    if !watched_dirs.is_empty() {
                        println!("Watched directories:");
                        for dir in watched_dirs {
                            println!("  - {}", dir.display());
                        }
                    }
                }
                DaemonStatus::Stopped => {
                    println!("Status: Stopped");
                }
                DaemonStatus::Error(msg) => {
                    println!("Status: Error");
                    println!("Error: {}", msg);
                }
            }
        }
    }

    Ok(())
}

fn handle_index(path: &PathBuf, db_path: &PathBuf, format: OutputFormat) -> Result<()> {
    let path = path.canonicalize().context("Invalid path")?;
    info!("Indexing: {}", path.display());

    let mut indexer = Indexer::new(db_path).context("Failed to create indexer")?;
    let stats = indexer
        .index_directory(&path)
        .context("Failed to index directory")?;

    output_index_stats(&stats, format);
    Ok(())
}

fn handle_reindex(
    path: Option<PathBuf>,
    file: Option<PathBuf>,
    db_path: &PathBuf,
    format: OutputFormat,
) -> Result<()> {
    let mut indexer = Indexer::new(db_path).context("Failed to create indexer")?;

    if let Some(file_path) = file {
        // Re-index single file
        let file_path = file_path.canonicalize().context("Invalid file path")?;
        let stats = indexer
            .index_file(&file_path)
            .context("Failed to index file")?;

        match format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "file": file_path.to_string_lossy(),
                        "symbols": stats.symbols,
                        "dependencies": stats.dependencies
                    })
                );
            }
            _ => {
                println!(
                    "Re-indexed {}: {} symbols, {} dependencies",
                    file_path.display(),
                    stats.symbols,
                    stats.dependencies
                );
            }
        }
    } else {
        // Re-index directory
        let path = path
            .unwrap_or_else(|| std::env::current_dir().unwrap())
            .canonicalize()
            .context("Invalid path")?;

        // Clear existing data first
        indexer.clear().context("Failed to clear index")?;

        let stats = indexer
            .index_directory(&path)
            .context("Failed to index directory")?;

        output_index_stats(&stats, format);
    }

    Ok(())
}

fn handle_query(query_type: QueryType, db_path: &PathBuf, format: OutputFormat) -> Result<()> {
    let db = Database::new(db_path).context("Failed to open database")?;
    let engine = QueryEngine::new(&db);

    match query_type {
        QueryType::Symbol { name, pattern } => {
            let symbols = if pattern {
                engine.search_symbols(&name)?
            } else {
                engine.find_symbol(&name)?
            };

            output_symbols(&name, &symbols, format);
        }
        QueryType::File { path } => {
            let path = path.canonicalize().context("Invalid file path")?;
            let symbols = engine.symbols_in_file(&path)?;

            output_symbols(&path.to_string_lossy(), &symbols, format);
        }
        QueryType::Dependencies { path, reverse } => {
            let path = path.canonicalize().context("Invalid file path")?;

            let deps = if reverse {
                engine.dependents_of(&path)?
            } else {
                engine.dependencies_from(&path)?
            };

            output_dependencies(&path.to_string_lossy(), reverse, &deps, format);
        }
        QueryType::AllDependencies { limit } => {
            let deps = engine.all_dependencies(limit)?;
            output_all_dependencies(&deps, format);
        }
        QueryType::HotFiles { limit } => {
            let files = engine.hot_files(limit)?;
            output_hot_files(limit, &files, format);
        }
        QueryType::Kind { kind, limit } => {
            let symbol_kind = SymbolKind::from_str(&kind)
                .ok_or_else(|| anyhow::anyhow!("Unknown symbol kind: {}", kind))?;

            let symbols = engine.symbols_by_kind(symbol_kind, limit)?;
            output_symbols(&format!("kind:{}", kind), &symbols, format);
        }
        QueryType::Callers { name } => {
            let calls = engine.callers(&name)?;
            output_callers(&name, &calls, format);
        }
        QueryType::Callees { name } => {
            let calls = engine.callees(&name)?;
            output_callees(&name, &calls, format);
        }
        QueryType::Implements { trait_name } => {
            let impls = engine.implementors(&trait_name)?;
            output_implementors(&trait_name, &impls, format);
        }
        QueryType::Traits { type_name } => {
            let impls = engine.traits_for(&type_name)?;
            output_traits(&type_name, &impls, format);
        }
        QueryType::Usages { name, kind, in_file } => {
            let usages = if let Some(kind_str) = kind {
                let usage_kind = UsageKind::from_str(&kind_str)
                    .ok_or_else(|| anyhow::anyhow!("Unknown usage kind: {}", kind_str))?;
                engine.usages_by_kind(&name, usage_kind)?
            } else if let Some(file_path) = in_file {
                let path = file_path.canonicalize().context("Invalid file path")?;
                engine.usages_in_file(&path)?
            } else {
                engine.usages(&name)?
            };
            output_usages(&name, &usages, format);
        }
        QueryType::Unused { in_file } => {
            let path = in_file
                .as_ref()
                .map(|p| p.canonicalize())
                .transpose()
                .context("Invalid file path")?;
            let symbols = engine.unused_symbols(path.as_deref())?;
            output_unused(path.as_deref(), &symbols, format);
        }
    }

    Ok(())
}

fn handle_stats(db_path: &PathBuf, format: OutputFormat) -> Result<()> {
    let db = Database::new(db_path).context("Failed to open database")?;
    let stats = db.get_stats()?;

    match format {
        OutputFormat::Json => {
            let result = StatsResult {
                total_files: stats.total_files,
                total_symbols: stats.total_symbols,
                total_dependencies: stats.total_dependencies,
                total_calls: stats.total_calls,
                total_implementations: stats.total_implementations,
                total_usages: stats.total_usages,
                languages: stats
                    .languages
                    .iter()
                    .map(|(lang, count)| {
                        let percentage = if stats.total_files > 0 {
                            (*count as f64 / stats.total_files as f64) * 100.0
                        } else {
                            0.0
                        };
                        LanguageStat {
                            language: lang.clone(),
                            file_count: *count,
                            percentage,
                        }
                    })
                    .collect(),
                database_size: stats.format_size(),
                last_indexed: stats.format_last_indexed(),
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("Index Statistics");
            println!("================");
            println!("Total files indexed: {}", stats.total_files);
            println!("Total symbols: {}", stats.total_symbols);
            println!("Total dependencies: {}", stats.total_dependencies);
            println!("Total calls: {}", stats.total_calls);
            println!("Total implementations: {}", stats.total_implementations);
            println!("Total usages: {}", stats.total_usages);

            if !stats.languages.is_empty() {
                println!("\nLanguages:");
                for (lang, count) in &stats.languages {
                    let pct = if stats.total_files > 0 {
                        (*count as f64 / stats.total_files as f64) * 100.0
                    } else {
                        0.0
                    };
                    println!("  {} - {} files ({:.1}%)", lang, count, pct);
                }
            }

            println!("\nDatabase size: {}", stats.format_size());
            println!("Last indexed: {}", stats.format_last_indexed());
        }
    }

    Ok(())
}

fn handle_clear(confirm: bool, db_path: &PathBuf) -> Result<()> {
    if !confirm {
        eprintln!("Use --confirm to clear the index");
        std::process::exit(1);
    }

    let mut indexer = Indexer::new(db_path).context("Failed to create indexer")?;
    indexer.clear().context("Failed to clear index")?;

    println!("Index cleared");
    Ok(())
}

fn handle_export(output: &PathBuf, db_path: &PathBuf) -> Result<()> {
    let db = Database::new(db_path).context("Failed to open database")?;
    let engine = QueryEngine::new(&db);

    // Export hot files as a proxy for all files
    // In a real implementation, we'd iterate all files
    let files = engine.hot_files(10000)?;

    let export = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "files": files.into_iter().map(FileResult::from).collect::<Vec<_>>(),
    });

    std::fs::write(output, serde_json::to_string_pretty(&export)?)?;
    println!("Exported to {}", output.display());

    Ok(())
}

fn output_index_stats(stats: &DirectoryIndexStats, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "files_indexed": stats.files_indexed,
                    "symbols_found": stats.symbols_found,
                    "dependencies_found": stats.dependencies_found,
                    "calls_found": stats.calls_found,
                    "implementations_found": stats.implementations_found,
                    "usages_found": stats.usages_found,
                    "errors": stats.errors
                })
            );
        }
        _ => {
            println!("Indexing complete:");
            println!("  Files indexed: {}", stats.files_indexed);
            println!("  Symbols found: {}", stats.symbols_found);
            println!("  Dependencies: {}", stats.dependencies_found);
            println!("  Calls: {}", stats.calls_found);
            println!("  Implementations: {}", stats.implementations_found);
            println!("  Usages: {}", stats.usages_found);
            if stats.errors > 0 {
                println!("  Errors: {}", stats.errors);
            }
        }
    }
}

fn output_symbols(
    query: &str,
    symbols: &[code_index::Symbol],
    format: OutputFormat,
) {
    match format {
        OutputFormat::Json => {
            let result = SymbolQueryResult {
                query: query.to_string(),
                count: symbols.len(),
                results: symbols.iter().cloned().map(SymbolResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for s in symbols {
                println!("{}:{}", s.file_path.display(), s.line_start);
            }
        }
        OutputFormat::Human => {
            if symbols.is_empty() {
                println!("No symbols found for: {}", query);
            } else {
                println!("Found {} symbol(s) for '{}':\n", symbols.len(), query);
                for s in symbols {
                    println!(
                        "  {} {} ({})",
                        s.kind.as_str(),
                        s.name,
                        s.visibility.as_str()
                    );
                    println!("    {}:{}-{}", s.file_path.display(), s.line_start, s.line_end);
                    if let Some(sig) = &s.signature {
                        println!("    {}", sig);
                    }
                    if let Some(parent) = &s.parent {
                        println!("    parent: {}", parent);
                    }
                    println!();
                }
            }
        }
    }
}

fn output_dependencies(
    file: &str,
    reverse: bool,
    deps: &[code_index::Dependency],
    format: OutputFormat,
) {
    match format {
        OutputFormat::Json => {
            let result = DependencyQueryResult {
                file: file.to_string(),
                direction: if reverse { "to" } else { "from" }.to_string(),
                count: deps.len(),
                dependencies: deps.iter().cloned().map(DependencyResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for d in deps {
                if let Some(target) = &d.target_file {
                    println!("{}", target.display());
                } else if let Some(symbol) = &d.symbol_name {
                    println!("{}", symbol);
                }
            }
        }
        OutputFormat::Human => {
            let direction = if reverse { "Dependents of" } else { "Dependencies from" };
            if deps.is_empty() {
                println!("No dependencies found for: {}", file);
            } else {
                println!("{} {} ({} found):\n", direction, file, deps.len());
                for d in deps {
                    print!("  [{}] ", d.kind.as_str());
                    if let Some(symbol) = &d.symbol_name {
                        print!("{}", symbol);
                    }
                    if let Some(target) = &d.target_file {
                        print!(" -> {}", target.display());
                    }
                    println!(" (line {})", d.line_number);
                }
            }
        }
    }
}

fn output_all_dependencies(deps: &[code_index::Dependency], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let results: Vec<DependencyResult> =
                deps.iter().cloned().map(DependencyResult::from).collect();
            println!("{}", serde_json::to_string_pretty(&results).unwrap());
        }
        OutputFormat::Compact => {
            for d in deps {
                let source = d.source_file.display();
                let target = d
                    .target_file
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .or_else(|| d.symbol_name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                println!("{} -> {}", source, target);
            }
        }
        OutputFormat::Human => {
            if deps.is_empty() {
                println!("No dependencies in index");
            } else {
                println!("All dependencies ({} found):\n", deps.len());
                // Group by source file
                let mut current_source: Option<&std::path::Path> = None;
                for d in deps {
                    if current_source != Some(d.source_file.as_path()) {
                        if current_source.is_some() {
                            println!();
                        }
                        println!("{}:", d.source_file.display());
                        current_source = Some(d.source_file.as_path());
                    }
                    print!("  [{}] ", d.kind.as_str());
                    if let Some(symbol) = &d.symbol_name {
                        print!("{}", symbol);
                    }
                    println!(" (line {})", d.line_number);
                }
            }
        }
    }
}

fn output_hot_files(limit: usize, files: &[code_index::FileMetadata], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = HotFilesResult {
                limit,
                count: files.len(),
                files: files.iter().cloned().map(FileResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for f in files {
                println!("{}", f.path.display());
            }
        }
        OutputFormat::Human => {
            if files.is_empty() {
                println!("No files in index");
            } else {
                println!("Hot files (top {}):\n", limit);
                for (i, f) in files.iter().enumerate() {
                    println!(
                        "  {}. {} (score: {:.1}, changes: {}, {} lines)",
                        i + 1,
                        f.path.display(),
                        f.hotness_score,
                        f.change_count,
                        f.lines_of_code
                    );
                }
            }
        }
    }
}

fn output_callers(callee: &str, calls: &[code_index::Call], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = CallersResult {
                callee: callee.to_string(),
                count: calls.len(),
                callers: calls.iter().cloned().map(CallResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for c in calls {
                println!("{}:{}:{}", c.call_file.display(), c.call_line, c.caller_name);
            }
        }
        OutputFormat::Human => {
            if calls.is_empty() {
                println!("No callers found for: {}", callee);
            } else {
                println!("Callers of '{}' ({} found):\n", callee, calls.len());
                for c in calls {
                    let async_marker = if c.is_async { " (async)" } else { "" };
                    let method_marker = if c.is_method { "." } else { "" };
                    println!(
                        "  {} ({}:{})",
                        c.caller_name,
                        c.call_file.display(),
                        c.call_line
                    );
                    println!(
                        "    -> {}{}{}\n",
                        method_marker, c.callee_name, async_marker
                    );
                }
            }
        }
    }
}

fn output_callees(caller: &str, calls: &[code_index::Call], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = CalleesResult {
                caller: caller.to_string(),
                count: calls.len(),
                callees: calls.iter().cloned().map(CallResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for c in calls {
                println!("{}", c.callee_name);
            }
        }
        OutputFormat::Human => {
            if calls.is_empty() {
                println!("No calls found from: {}", caller);
            } else {
                println!("Functions called by '{}' ({} found):\n", caller, calls.len());
                for c in calls {
                    let async_marker = if c.is_async { " (async)" } else { "" };
                    let method_marker = if c.is_method { "(method) " } else { "" };
                    println!(
                        "  {}{}{}",
                        method_marker, c.callee_name, async_marker
                    );
                    println!(
                        "    at {}:{}\n",
                        c.call_file.display(),
                        c.call_line
                    );
                }
            }
        }
    }
}

fn output_implementors(
    trait_name: &str,
    impls: &[code_index::Implementation],
    format: OutputFormat,
) {
    match format {
        OutputFormat::Json => {
            let result = ImplementorsResult {
                trait_name: trait_name.to_string(),
                count: impls.len(),
                implementors: impls.iter().cloned().map(ImplementationResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for i in impls {
                println!("{}:{}:{}", i.impl_file.display(), i.impl_line, i.implementor_name);
            }
        }
        OutputFormat::Human => {
            if impls.is_empty() {
                println!("No implementations found for trait: {}", trait_name);
            } else {
                println!(
                    "Implementations of '{}' ({} found):\n",
                    trait_name,
                    impls.len()
                );
                for i in impls {
                    println!("  {} ({}:{})", i.implementor_name, i.impl_file.display(), i.impl_line);
                }
            }
        }
    }
}

fn output_traits(type_name: &str, impls: &[code_index::Implementation], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = TraitsResult {
                type_name: type_name.to_string(),
                count: impls.len(),
                traits: impls.iter().cloned().map(ImplementationResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for i in impls {
                println!("{}", i.trait_name);
            }
        }
        OutputFormat::Human => {
            if impls.is_empty() {
                println!("No traits found for type: {}", type_name);
            } else {
                println!(
                    "Traits implemented by '{}' ({} found):\n",
                    type_name,
                    impls.len()
                );
                for i in impls {
                    println!("  {} ({}:{})", i.trait_name, i.impl_file.display(), i.impl_line);
                }
            }
        }
    }
}

fn output_usages(symbol: &str, usages: &[code_index::Usage], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = UsagesQueryResult {
                symbol: symbol.to_string(),
                count: usages.len(),
                usages: usages.iter().cloned().map(UsageResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for u in usages {
                println!(
                    "{}:{}:{}",
                    u.usage_file.display(),
                    u.usage_line,
                    u.kind.as_str()
                );
            }
        }
        OutputFormat::Human => {
            if usages.is_empty() {
                println!("No usages found for: {}", symbol);
            } else {
                println!("Usages of '{}' ({} found):\n", symbol, usages.len());

                // Group by usage kind
                let mut by_kind: std::collections::HashMap<&str, Vec<&code_index::Usage>> =
                    std::collections::HashMap::new();
                for u in usages {
                    by_kind.entry(u.kind.as_str()).or_default().push(u);
                }

                for (kind, kind_usages) in by_kind {
                    println!("  As {} ({}):", kind, kind_usages.len());
                    for u in kind_usages {
                        let context = u
                            .context_name
                            .as_ref()
                            .map(|c| format!(" in {}", c))
                            .unwrap_or_default();
                        println!(
                            "    {}:{}{}",
                            u.usage_file.display(),
                            u.usage_line,
                            context
                        );
                    }
                    println!();
                }
            }
        }
    }
}

fn output_unused(file: Option<&std::path::Path>, symbols: &[code_index::Symbol], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let result = UnusedResult {
                file: file.map(|p| p.to_string_lossy().to_string()),
                count: symbols.len(),
                symbols: symbols.iter().cloned().map(SymbolResult::from).collect(),
            };
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Compact => {
            for s in symbols {
                println!("{}:{}", s.file_path.display(), s.line_start);
            }
        }
        OutputFormat::Human => {
            if symbols.is_empty() {
                if let Some(f) = file {
                    println!("No unused symbols found in: {}", f.display());
                } else {
                    println!("No unused symbols found");
                }
            } else {
                let scope = file
                    .map(|f| format!(" in {}", f.display()))
                    .unwrap_or_default();
                println!("Potentially unused symbols{} ({} found):\n", scope, symbols.len());
                for s in symbols {
                    println!(
                        "  {} {} ({}:{})",
                        s.kind.as_str(),
                        s.name,
                        s.file_path.display(),
                        s.line_start
                    );
                }
            }
        }
    }
}
