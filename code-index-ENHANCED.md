# CodeIndex Enhancement Specification

**Tool Location:** `/home/mason/.cargo/bin/code-index`
**Purpose:** Persistent semantic cache for AI agents - indexes codebases for fast symbol lookup and relationship analysis
**Current Version:** Solid foundation with symbol indexing and basic dependency tracking

---

## Current Capabilities (Working Well)

- ✅ Symbol indexing (functions, structs, traits, etc.)
- ✅ Symbol lookup by name (`code-index query symbol "X"`)
- ✅ File dependency listing (`code-index query dependencies file.rs`)
- ✅ Symbols by kind (`code-index query kind function`)
- ✅ File watching daemon mode
- ✅ JSON output option

---

## Problem Statement

The current CodeIndex provides **static lookups** but lacks **relationship traversal**:

1. **No call graph:** Cannot answer "what functions call X?" or "what does X call?"
2. **No inheritance/impl tracking:** Cannot answer "what implements trait X?"
3. **No usage analysis:** Cannot answer "where is type X used?"
4. **No cross-file relationships:** Dependencies show imports, not actual usage
5. **Limited query language:** Cannot combine conditions

---

## Required Enhancements

### 1. Call Graph Support (HIGH PRIORITY)

**Current behavior:** No call relationship tracking
**Required behavior:** Track and query function calls

#### 1.1 Index Call Relationships

During indexing, extract call relationships:

```rust
// When parsing this code:
fn main() {
    let config = Config::load();
    let coordinator = SearchCoordinator::new(&config);
    let results = coordinator.search(&query);
}

// Index these relationships:
// main -> Config::load
// main -> SearchCoordinator::new
// main -> SearchCoordinator::search
```

**Storage schema addition:**
```sql
CREATE TABLE calls (
    id INTEGER PRIMARY KEY,
    caller_symbol_id INTEGER REFERENCES symbols(id),
    callee_name TEXT NOT NULL,
    callee_symbol_id INTEGER REFERENCES symbols(id), -- NULL if external
    call_file TEXT NOT NULL,
    call_line INTEGER NOT NULL,
    call_column INTEGER NOT NULL,
    is_method BOOLEAN DEFAULT FALSE,
    is_async BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_calls_caller ON calls(caller_symbol_id);
CREATE INDEX idx_calls_callee ON calls(callee_symbol_id);
CREATE INDEX idx_calls_callee_name ON calls(callee_name);
```

#### 1.2 Query Call Graph

```bash
# Find all callers of a function
code-index query callers "search"

# Output:
# Callers of 'search' (3 found):
#   main (src/main.rs:25)
#     → coordinator.search(&query)
#
#   test_basic_search (src/search/text.rs:250)
#     → searcher.search(&query)
#
#   hybrid_search (src/search/hybrid.rs:95)
#     → self.text_searcher.search(&text_query)


# Find all functions called by a function
code-index query callees "main"

# Output:
# Functions called by 'main' (5 found):
#   Config::load (src/config.rs:180)
#   SearchCoordinator::new (src/search/mod.rs:45)
#   SearchCoordinator::search (src/search/mod.rs:52)
#   RankingCoordinator::rank (src/rank/mod.rs:35)
#   FormatterCoordinator::format (src/format/mod.rs:60)


# Transitive callers (who calls functions that call X)
code-index query callers "search" --depth 2

# Call graph visualization
code-index query call-graph "main" --format mermaid
```

#### 1.3 Implementation Approach

1. **During parsing:** Use tree-sitter to identify:
   - Function call expressions: `foo()`, `obj.method()`, `Type::method()`
   - Method calls on known types
   - Async calls (`.await`)

2. **Resolution:**
   - Try to resolve callee to a known symbol in the index
   - Store unresolved calls by name (for external dependencies)

3. **Query optimization:**
   - Index by both caller and callee for bidirectional queries
   - Support depth-limited traversal to prevent explosion

### 2. Trait/Interface Implementation Tracking

**Current behavior:** No implementation relationship tracking
**Required behavior:** Track what implements what

#### 2.1 Index Implementation Relationships

```rust
// When parsing:
impl Searcher for TextSearcher {
    fn search(&self, query: &Query) -> Result<Vec<SearchResult>> { ... }
}

// Index:
// TextSearcher implements Searcher
// TextSearcher::search implements Searcher::search
```

**Storage schema addition:**
```sql
CREATE TABLE implementations (
    id INTEGER PRIMARY KEY,
    implementor_id INTEGER REFERENCES symbols(id),  -- TextSearcher
    trait_id INTEGER REFERENCES symbols(id),         -- Searcher (nullable if external)
    trait_name TEXT NOT NULL,                        -- "Searcher"
    impl_file TEXT NOT NULL,
    impl_line INTEGER NOT NULL
);

CREATE INDEX idx_impl_implementor ON implementations(implementor_id);
CREATE INDEX idx_impl_trait ON implementations(trait_id);
CREATE INDEX idx_impl_trait_name ON implementations(trait_name);
```

#### 2.2 Query Implementations

```bash
# Find all implementations of a trait
code-index query implements "Searcher"

# Output:
# Implementations of 'Searcher' (4 found):
#   TextSearcher (src/search/text.rs:171)
#   StructuralSearcher (src/search/structural.rs:318)
#   GraphSearcher (src/search/graph.rs:330)
#   HybridSearcher (src/search/hybrid.rs:132)


# Find what traits a type implements
code-index query traits "TextSearcher"

# Output:
# Traits implemented by 'TextSearcher':
#   Searcher (src/search/mod.rs:24)
#   Default (std) - via derive
#   Send (std) - auto trait
#   Sync (std) - auto trait


# Find implementations of a specific method
code-index query implements "Searcher::search"
```

### 3. Usage/Reference Tracking

**Current behavior:** Only tracks imports
**Required behavior:** Track where symbols are actually used

#### 3.1 Index References

```rust
// When parsing:
fn process(result: SearchResult) {
    println!("{}", result.snippet);
}

// Index references:
// SearchResult used at process:1 (type annotation)
// SearchResult.snippet used at process:2 (field access)
```

**Storage schema addition:**
```sql
CREATE TABLE usages (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER REFERENCES symbols(id),
    symbol_name TEXT NOT NULL,
    usage_kind TEXT NOT NULL,  -- 'type', 'call', 'field_access', 'variable', 'import'
    usage_file TEXT NOT NULL,
    usage_line INTEGER NOT NULL,
    usage_column INTEGER NOT NULL,
    context_symbol_id INTEGER REFERENCES symbols(id)  -- enclosing function/method
);

CREATE INDEX idx_usages_symbol ON usages(symbol_id);
CREATE INDEX idx_usages_name ON usages(symbol_name);
CREATE INDEX idx_usages_file ON usages(usage_file);
```

#### 3.2 Query Usages

```bash
# Find all usages of a type
code-index query usages "SearchResult"

# Output:
# Usages of 'SearchResult' (23 found):
#
#   As type annotation (8):
#     src/search/mod.rs:26 - fn search(...) -> Result<Vec<SearchResult>>
#     src/search/text.rs:95 - fn search_file(...) -> Result<Vec<SearchResult>>
#     ...
#
#   As return value (6):
#     src/search/text.rs:120 - results.push(SearchResult { ... })
#     ...
#
#   Field access (9):
#     src/format/human.rs:45 - result.file.display()
#     src/format/json.rs:32 - &result.snippet
#     ...


# Find usages in a specific file
code-index query usages "Query" --in src/cli.rs

# Find unused symbols (defined but never referenced)
code-index query unused
```

### 4. Advanced Query Language

**Current behavior:** Simple single-condition queries
**Required behavior:** Composable query conditions

#### 4.1 Query Syntax

```bash
# Combine conditions
code-index query "kind:function AND name:search"
code-index query "kind:struct AND file:src/types.rs"

# Negation
code-index query "kind:function AND NOT name:test_*"

# Regex patterns
code-index query "name:/^test_.*/"

# Scope limiting
code-index query "name:search" --in src/search/

# Multiple kinds
code-index query "kind:function,method AND public:true"
```

#### 4.2 Query Examples

```bash
# Find all public functions in a module
code-index query "kind:function AND public:true" --in src/search/

# Find all async functions
code-index query "kind:function AND async:true"

# Find all structs with a specific field
code-index query "kind:struct AND has_field:relevance_score"

# Find all test functions
code-index query "kind:function AND name:/^test_/"

# Complex: Find public functions that call database methods
code-index query "kind:function AND public:true AND calls:query"
```

### 5. Incremental Indexing Improvements

**Current behavior:** Basic file watching
**Required behavior:** Smart incremental updates with dependency tracking

#### 5.1 Dependency-Aware Updates

```bash
# When a file changes, also re-check dependents
code-index daemon start --propagate-changes

# Example scenario:
# 1. types.rs changes (SearchResult struct modified)
# 2. code-index detects change to types.rs
# 3. code-index identifies files importing SearchResult
# 4. code-index re-indexes affected files
# 5. Updates call graph and usage references
```

#### 5.2 Index Health Commands

```bash
# Check index consistency
code-index health

# Output:
# Index Health Check
# ─────────────────
# Symbols: 432 (all valid)
# References: 169 (3 orphaned - cleaning)
# Calls: 287 (all valid)
# Last full index: 2 hours ago
# Files watched: 25
# Pending updates: 0
#
# Status: HEALTHY


# Force re-index of specific files
code-index reindex src/search/

# Compact/optimize database
code-index optimize
```

### 6. Context-Optimized Output

**Current behavior:** Human-readable or JSON
**Required behavior:** AI-agent optimized output modes

#### 6.1 AI Context Mode

```bash
# Output optimized for AI agent context
code-index query symbol "Searcher" --format ai-context

# Output:
# SYMBOL: Searcher
# KIND: trait
# FILE: src/search/mod.rs:24-33
# VISIBILITY: public
#
# DEFINITION:
# ```rust
# pub trait Searcher: Send + Sync {
#     fn search(&self, query: &Query) -> Result<Vec<SearchResult>>;
#     fn can_handle(&self, query: &Query) -> bool;
#     fn name(&self) -> &'static str;
# }
# ```
#
# IMPLEMENTATIONS: 4
#   - TextSearcher (src/search/text.rs:171)
#   - StructuralSearcher (src/search/structural.rs:318)
#   - GraphSearcher (src/search/graph.rs:330)
#   - HybridSearcher (src/search/hybrid.rs:132)
#
# USAGES: 12 references across 5 files
#
# RELATED:
#   - SearchCoordinator uses Searcher (composition)
#   - Query is parameter type
#   - SearchResult is return type
```

#### 6.2 Diff Mode

```bash
# Show what changed since last query
code-index query symbol "Searcher" --since "1 hour ago"

# Output:
# Changes to 'Searcher' since 1 hour ago:
#   [MODIFIED] src/search/mod.rs:24
#     - Added method: fn supports_parallel(&self) -> bool
#   [NEW IMPL] AsyncSearcher (src/search/async.rs:45)
```

### 7. Export/Integration Features

#### 7.1 LSP Integration

```bash
# Start as language server protocol backend
code-index lsp

# Provides:
# - Go to definition
# - Find all references
# - Find implementations
# - Call hierarchy
```

#### 7.2 Export Formats

```bash
# Export to various formats for integration
code-index export --format dot > graph.dot        # Graphviz
code-index export --format mermaid > graph.mmd    # Mermaid diagrams
code-index export --format json > index.json      # Full JSON dump
code-index export --format sqlite > index.db      # SQLite copy
```

---

## New Commands Summary

```bash
# Call graph queries
code-index query callers <symbol>
code-index query callees <symbol>
code-index query call-graph <symbol> [--depth N] [--format mermaid|dot|json]

# Implementation queries
code-index query implements <trait>
code-index query traits <type>

# Usage queries
code-index query usages <symbol> [--in <path>] [--kind <usage_kind>]
code-index query unused [--in <path>]

# Advanced queries
code-index query "<query_expression>" [--in <path>]

# Index management
code-index health
code-index optimize
code-index reindex [path]

# Export
code-index export --format <dot|mermaid|json|sqlite>

# LSP mode
code-index lsp [--port N]
```

---

## Database Schema Additions

```sql
-- Call relationships
CREATE TABLE calls (
    id INTEGER PRIMARY KEY,
    caller_symbol_id INTEGER REFERENCES symbols(id),
    callee_name TEXT NOT NULL,
    callee_symbol_id INTEGER,
    call_file TEXT NOT NULL,
    call_line INTEGER NOT NULL,
    call_column INTEGER NOT NULL,
    is_method BOOLEAN DEFAULT FALSE,
    is_async BOOLEAN DEFAULT FALSE
);

-- Trait implementations
CREATE TABLE implementations (
    id INTEGER PRIMARY KEY,
    implementor_id INTEGER REFERENCES symbols(id),
    trait_id INTEGER,
    trait_name TEXT NOT NULL,
    impl_file TEXT NOT NULL,
    impl_line INTEGER NOT NULL
);

-- Symbol usages/references
CREATE TABLE usages (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER,
    symbol_name TEXT NOT NULL,
    usage_kind TEXT NOT NULL,
    usage_file TEXT NOT NULL,
    usage_line INTEGER NOT NULL,
    usage_column INTEGER NOT NULL,
    context_symbol_id INTEGER
);

-- Indexes for performance
CREATE INDEX idx_calls_caller ON calls(caller_symbol_id);
CREATE INDEX idx_calls_callee ON calls(callee_symbol_id);
CREATE INDEX idx_calls_callee_name ON calls(callee_name);
CREATE INDEX idx_impl_implementor ON implementations(implementor_id);
CREATE INDEX idx_impl_trait ON implementations(trait_id);
CREATE INDEX idx_impl_trait_name ON implementations(trait_name);
CREATE INDEX idx_usages_symbol ON usages(symbol_id);
CREATE INDEX idx_usages_name ON usages(symbol_name);
CREATE INDEX idx_usages_file ON usages(usage_file);
```

---

## Implementation Priority

1. **CRITICAL:** Call graph support (callers/callees queries)
2. **HIGH:** Implementation tracking (implements/traits queries)
3. **HIGH:** Usage tracking (usages query)
4. **MEDIUM:** Advanced query language
5. **MEDIUM:** AI-context output format
6. **LOW:** LSP integration
7. **LOW:** Export formats

---

## Success Criteria

After enhancement, CodeIndex should enable an AI agent to:

1. **Navigate code relationships:**
   - "What calls this function?" → Instant answer
   - "What does this function call?" → Instant answer
   - "What implements this trait?" → Instant answer

2. **Understand impact of changes:**
   - "If I change this function signature, what breaks?" → List all callers
   - "If I add a method to this trait, what needs updating?" → List all implementors

3. **Find patterns:**
   - "Show me all async functions that call the database" → Query expression
   - "Find unused code" → Unused symbol detection

4. **Reduce token usage:**
   - Single query replaces multiple grep/read operations
   - Structured output ready for AI consumption

---

## Technical Notes

- Tree-sitter already parses AST - extend to extract call sites
- SQLite already used - add new tables with proper indexing
- Consider memory-mapped indexes for large codebases
- Call resolution may be imperfect (dynamic dispatch, generics) - document limitations
- Support Rust, TypeScript, Python, Go initially

---

## Integration with context-query

CodeIndex enhancements directly support context-query's graph search mode:

```rust
// In context-query's GraphSearcher:
// - Use code-index callers query for --show-callers
// - Use code-index callees query for --show-callees
// - Use code-index implements query for trait analysis
// - Use code-index usages query for reference finding
```

---

## Additional Enhancements from Testing & Roadmap

### Priority 1: Incremental Indexing (CRITICAL)

**Problem**: Re-indexing entire codebase is wasteful
**Current**: Full re-index on every update
**Required**: Incremental updates with file mtime tracking

**Implementation**:
```rust
// Store file mtimes in database
CREATE TABLE file_mtimes (
    path TEXT PRIMARY KEY,
    mtime INTEGER NOT NULL,
    FOREIGN KEY(path) REFERENCES files(path)
);

// On update, compare mtimes
pub fn incremental_update(&mut self) -> Result<Vec<PathBuf>> {
    let changed_files = self.find_changed_files()?;
    for file in changed_files {
        self.reindex_file(&file)?;
    }
    Ok(changed_files)
}

// Daemon mode with incremental updates
code-index daemon start --watch . --incremental
```

**Impact**: 10-100x faster updates on large codebases

---

### Priority 2: Query Result Caching

**Implementation**:
```rust
// Cache in ~/.cache/ai-tools/query-cache/
// Key: hash(query + file_mtimes)
pub struct QueryCache {
    cache_dir: PathBuf,
    index_client: CodeIndexClient,
}

impl QueryCache {
    pub fn get_or_compute<T>(&self, query: &Query, compute: impl FnOnce() -> T) -> T {
        let key = self.compute_key(query);
        if let Some(cached) = self.load(key) {
            if self.is_valid(cached, query) {
                return cached.value;
            }
        }
        let result = compute();
        self.store(key, result);
        result
    }
}
```

**New Commands**:
```bash
# Cache management
code-index cache stats
code-index cache clear [--older-than <DAYS>]
code-index cache invalidate <pattern>
```

**Impact**: 10-100x faster repeated queries

---

### Priority 3: Better CLI Output Formatting

**Current**: Plain text output
**Enhanced**: Formatted with symbols and structure

```bash
# Before
code-index query --symbol login
# Plain text output

# After
code-index query --symbol login
# ✓ Found 3 occurrences of 'login'
#
# 📄 src/auth/login.rs:42
#    function login(credentials: Credentials) -> Result<User>
#    Called by: 2 functions
#    Dependencies: src/db/users.rs, src/utils/crypto.rs
```

**Implementation**: Use colored terminal output with `termcolor` crate

---

### Quick Wins

**QW1: Progress Bars**
```rust
use indicatif::{ProgressBar, ProgressStyle};

// During indexing
code-index index .
// ⠋ Indexing... [████████░░] 82% (82/100 files)
//   Current: src/main.rs
//   Symbols: 3,421 | Dependencies: 234
```

**QW2: Shell Completions**
```bash
# Auto-install completions
code-index completions install
# Auto-detects shell (bash/zsh/fish)
```

**QW3: JSON Schema Validation**
```bash
# Validate JSON output structure
code-index query --symbol "test" --format json --validate
```

---

### Future: Semantic Search Integration

**When implemented**:
```bash
# Find conceptually similar code using embeddings
code-index query --semantic "authentication and authorization"

# Database schema addition:
CREATE TABLE embeddings (
    symbol_id INTEGER PRIMARY KEY,
    embedding BLOB,  -- 384-dimensional float vector
    FOREIGN KEY(symbol_id) REFERENCES symbols(id)
);
```

**Uses**: Local embedding model (all-MiniLM-L6-v2) for vector similarity search

---

*This specification should be fed to an AI agent to implement the enhancements.*
