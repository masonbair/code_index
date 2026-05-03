-- code-index SQLite schema
-- Persistent semantic cache for AI agents

-- Symbols table: Functions, classes, types, etc.
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    signature TEXT,
    parent_symbol TEXT,
    visibility TEXT,
    language TEXT NOT NULL,
    indexed_at INTEGER NOT NULL,
    UNIQUE(name, file_path, line_start)
);

-- Dependencies table: Imports, calls, inheritance
CREATE TABLE IF NOT EXISTS dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    target_file TEXT,
    kind TEXT NOT NULL,
    symbol_name TEXT,
    line_number INTEGER,
    indexed_at INTEGER NOT NULL
);

-- Files metadata table
CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,
    change_count INTEGER DEFAULT 1,
    hotness_score REAL DEFAULT 0.0,
    language TEXT,
    lines_of_code INTEGER DEFAULT 0,
    indexed_at INTEGER NOT NULL,
    status TEXT DEFAULT 'indexed'
);

-- Project configuration table
CREATE TABLE IF NOT EXISTS project_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Call relationships table: tracks function/method calls
CREATE TABLE IF NOT EXISTS calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    caller_symbol_id INTEGER,
    callee_name TEXT NOT NULL,
    callee_symbol_id INTEGER,
    call_file TEXT NOT NULL,
    call_line INTEGER NOT NULL,
    call_column INTEGER NOT NULL,
    is_method BOOLEAN DEFAULT FALSE,
    is_async BOOLEAN DEFAULT FALSE,
    indexed_at INTEGER NOT NULL,
    FOREIGN KEY (caller_symbol_id) REFERENCES symbols(id) ON DELETE CASCADE,
    FOREIGN KEY (callee_symbol_id) REFERENCES symbols(id) ON DELETE SET NULL
);

-- Trait/interface implementations table
CREATE TABLE IF NOT EXISTS implementations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    implementor_id INTEGER,
    trait_id INTEGER,
    trait_name TEXT NOT NULL,
    impl_file TEXT NOT NULL,
    impl_line INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL,
    FOREIGN KEY (implementor_id) REFERENCES symbols(id) ON DELETE CASCADE,
    FOREIGN KEY (trait_id) REFERENCES symbols(id) ON DELETE SET NULL
);

-- Symbol usages/references table
CREATE TABLE IF NOT EXISTS usages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_id INTEGER,
    symbol_name TEXT NOT NULL,
    usage_kind TEXT NOT NULL,
    usage_file TEXT NOT NULL,
    usage_line INTEGER NOT NULL,
    usage_column INTEGER NOT NULL,
    context_symbol_id INTEGER,
    indexed_at INTEGER NOT NULL,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id) ON DELETE SET NULL,
    FOREIGN KEY (context_symbol_id) REFERENCES symbols(id) ON DELETE SET NULL
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
CREATE INDEX IF NOT EXISTS idx_symbols_language ON symbols(language);
CREATE INDEX IF NOT EXISTS idx_deps_source ON dependencies(source_file);
CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_file);
CREATE INDEX IF NOT EXISTS idx_deps_symbol ON dependencies(symbol_name);
CREATE INDEX IF NOT EXISTS idx_files_hotness ON files(hotness_score DESC);
CREATE INDEX IF NOT EXISTS idx_files_status ON files(status);

-- Call relationship indexes
CREATE INDEX IF NOT EXISTS idx_calls_caller ON calls(caller_symbol_id);
CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee_symbol_id);
CREATE INDEX IF NOT EXISTS idx_calls_callee_name ON calls(callee_name);
CREATE INDEX IF NOT EXISTS idx_calls_file ON calls(call_file);

-- Implementation indexes
CREATE INDEX IF NOT EXISTS idx_impl_implementor ON implementations(implementor_id);
CREATE INDEX IF NOT EXISTS idx_impl_trait ON implementations(trait_id);
CREATE INDEX IF NOT EXISTS idx_impl_trait_name ON implementations(trait_name);
CREATE INDEX IF NOT EXISTS idx_impl_file ON implementations(impl_file);

-- Usage indexes
CREATE INDEX IF NOT EXISTS idx_usages_symbol ON usages(symbol_id);
CREATE INDEX IF NOT EXISTS idx_usages_name ON usages(symbol_name);
CREATE INDEX IF NOT EXISTS idx_usages_file ON usages(usage_file);
CREATE INDEX IF NOT EXISTS idx_usages_kind ON usages(usage_kind);
CREATE INDEX IF NOT EXISTS idx_usages_context ON usages(context_symbol_id);
