# C.I.S. — Codebase Intelligence System

**Version:** 1.0.0  
**Status:** MVP Complete (Phases 1-10, 14 implemented)

## Overview

C.I.S. is a Rust-based codebase intelligence system that provides fast code navigation, search, dependency analysis, and context management for developers. It uses SQLite with FTS5 for storage and MCP (Model Context Protocol) for external AI integration.

## Features

- **Code Scanning:** Index files across multiple languages (Rust, Python, JavaScript, Go, TypeScript)
- **Symbol Search:** Find functions, classes, imports, and other symbols across the codebase
- **Dependency Graph:** Track file dependencies and perform impact analysis
- **Git Integration:** Branch detection, status, diff, file history
- **Context Memory:** Project and session memory with TTL, provenance tracking, and budget limits
- **MCP Server:** Expose tools to external AI via the Model Context Protocol
- **Context Export/Import:** Portable ZIP archives for sharing context between projects

## Quick Start

```bash
# Initialize C.I.S. in your project
cis init

# Scan and index your codebase
cis scan

# Search for symbols
cis search "FunctionName"

# Get project overview
cis status
```

## Commands

| Command | Description |
|---------|-------------|
| `cis init` | Initialize C.I.S. project |
| `cis scan` | Scan and index files |
| `cis status` | Show index statistics |
| `cis parse` | Parse a single file |
| `cis search` | Full-text search |
| `cis symbol` | Find symbol definitions |
| `cis deps` | Show file dependencies |
| `cis impact` | Show affected files |
| `cis entrypoints` | Show entry points |
| `cis cycles` | Check for circular dependencies |
| `cis git` | Git integration commands |
| `cis memory` | Manage project/session memory |
| `cis context` | Export/import context |
| `cis config` | Show/modify configuration |
| `cis doctor` | Run diagnostic checks |
| `cis mcp` | Start MCP server (for AI) |
| `cis run` | Execute terminal commands |

## Architecture

### Core Components

```
src/
├── cli/          # CLI argument parsing and command dispatch
├── config/       # Configuration management (TOML)
├── context/      # Context export/import (ZIP archives)
├── diagnostics/  # Health checks and diagnostics
├── git/          # Git CLI integration
├── index/        # SQLite index with FTS5
├── memory/       # Project/session memory system
├── migration/    # Database schema migrations
├── mcp/          # MCP server (JSON-RPC over stdio)
├── parser/       # Code parsers (Rust, Python, JS, Go)
├── project/      # Project discovery and initialization
├── release/      # Release management
├── scanner/      # File scanning and indexing
├── security/     # Security policies
└── terminal/     # Terminal execution with sandboxing
```

### Memory Architecture

```
project/.cis/
├── config.toml
├── project.db
└── context/
    ├── project/           ← durable project memory
    └── sessions/          ← temporary session memory
```

### Security Model

1. **Permission check** — Allowlist/denylist for commands
2. **Path containment** — Commands execute within project directory
3. **Environment restrictions** — No dangerous env vars
4. **Resource limits** — Timeouts, output limits
5. **Audit logging** — All executions logged
6. **Secret redaction** — Common secret patterns masked

## Configuration

Configuration is stored in `.cis/config.toml`:

```toml
[general]
project_name = "my-project"
ignore_dirs = ["target", "node_modules"]
max_file_size_bytes = 1048576

[scanner]
languages = ["Rust", "Python", "JavaScript"]
enable_incremental = true
respect_gitignore = true

[context]
max_session_context_bytes = 10485760  # 10MB
max_sessions = 50
max_total_context_bytes = 524288000   # 500MB
max_context_file_bytes = 1048576      # 1MB
session_ttl_seconds = 604800          # 7 days
```

## MCP Server

Start the MCP server:

```bash
cis mcp
```

Available tools: `project_overview`, `search`, `file_context`, `symbol_context`, `dependency_context`, `impact_analysis`, `memory_context`, `session_context`, `git_context`, `diagnostics`, `run_command`

## Development

### Building

```bash
cargo build
cargo test
cargo clippy --all-targets
```

### Database Migrations

Migrations run automatically on startup. Current schema version: 3

```bash
cargo test migration
```

## License

See LICENSE file.
