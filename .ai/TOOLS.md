# Available AI Agent Tools

**Auto-generated:** 2026-04-21
**System:** Arch Linux
**Tools Detected:** 0 of 4 installed

This file is a registry of custom tools available on this system for AI agent workflows.

---

## Tool Status


❌ **CodeSummarizer** - NOT INSTALLED

❌ **ContextQuery** - NOT INSTALLED

❌ **CodeIndex** - NOT INSTALLED

❌ **ContextPacker** - NOT INSTALLED


---

## Tool Descriptions & Usage


### 1. CodeSummarizer

**Status:** ❌ Not Installed

**Purpose:** Generates hierarchical context maps of the codebase for AI agents

**Usage:**
```bash
code-summarizer --project-root . --output .ai/context/
```


**Output:**
- `.ai/context/ARCHITECTURE.md` - High-level system design
- `.ai/context/MODULE_MAPS/` - Per-module breakdowns
- `.ai/context/DEPENDENCY_GRAPH.md` - Import/call relationships

**When to use:** At project start, after major refactors, or when context feels stale.

**AI Agent Note:** Run this BEFORE doing broad codebase analysis to get efficient, structured context.


---


### 2. ContextQuery

**Status:** ❌ Not Installed

**Purpose:** Structure-aware code search combining text, AST patterns, and graph traversal

**Usage:**
```bash
context-query --pattern "async function.*database" --type structural
```


**Output:** JSON with code snippets, file:line locations, relevance scores, dependency info.

**AI Agent Note:** Use this instead of basic grep/ripgrep for code search - it understands structure.


---


### 3. CodeIndex

**Status:** ❌ Not Installed

**Purpose:** Persistent semantic index (symbol table, dependency graph, file metadata)

**Usage:**
```bash
code-index daemon --watch .
```


**Output:** JSON with symbols, dependencies, metadata.

**AI Agent Note:** This is the backend for ContextQuery and CodeSummarizer. It's likely already running.


---


### 4. ContextPacker

**Status:** ❌ Not Installed

**Purpose:** Smart context window packing - assembles relevant code within token budget

**Usage:**
```bash
context-packer --query "implement feature" --budget 8000 --format claude
```


**Output:** Pre-formatted context optimized for your token budget and target model.

**AI Agent Note:** Use this when you need to understand a feature but want to stay within token limits.


---



## Best Practices for AI Agents

1. **Start with CodeSummarizer:** Run it first to get high-level context (~200 tokens)
2. **Use ContextQuery for specifics:** Drill down to specific code with structure-aware search
3. **Let ContextPacker manage tokens:** When context budget is tight, use it to prioritize
4. **Trust the index:** CodeIndex is faster than re-parsing - use it for symbol/dependency lookups

---

## Tool Installation Status

If any tools show as "NOT INSTALLED", they can be built from specs or request installation instructions from the user.

**Current Status:** 0/4 tools installed
