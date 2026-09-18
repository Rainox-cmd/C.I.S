# Connecting an AI Agent to C.I.S via MCP

C.I.S. (Context & Intelligence System) supports the Model Context Protocol (MCP), allowing any compatible AI agent (like Claude Desktop, Google Gemini, or custom Antigravity agents) to securely connect and query your codebase structure, graphs, and diagnostics.

## Setting Up the MCP Connection

To connect an AI agent, you just need to add the `cis` binary as an MCP server in your agent's configuration file (e.g., `mcp_config.json`, `claude_desktop_config.json`, etc.).

### Example Configuration

Add the following to your agent's MCP configuration:

```json
{
  "mcpServers": {
    "cis": {
      "command": "C:/Users/Admin/Desktop/C.I.S/target/release/cis.exe",
      "args": ["mcp"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Once configured, the AI agent will automatically launch C.I.S. in the background and have access to all the exposed tools!

---

# C.I.S. Manual CLI Commands

You don't need an AI agent to use C.I.S.! You can execute all of its graph, dependency, and impact analysis capabilities manually using the Command Line Interface (CLI). 

Here are the primary commands you can use in your terminal:

### 1. Initialization & Setup

Before analyzing a project, initialize and scan it.

- **`cis init`**
  Initializes a new C.I.S. database inside a `.cis/` hidden folder in the current directory.
  
- **`cis scan`**
  Scans all files in the current directory, detects languages, builds the Abstract Syntax Trees (ASTs) using Tree-sitter, and populates the SQLite database with file boundaries and relationships.

- **`cis status`**
  Displays the current health of the database, the total number of files scanned, language statistics, and SQLite lock statuses.

### 2. Architecture & Graphs

- **`cis analyze`**
  Generates a comprehensive structured overview of the project, including component groupings, entry points, and graph complexities (outputs in JSON).

- **`cis deps <file_path>`**
  *Example: `cis deps src/main.rs`*
  Queries the dependency graph to show exactly what files the specified file depends on.

- **`cis impact <file_path>`**
  *Example: `cis impact src/config.rs`*
  Queries the impact graph to show what files depend on the specified file (i.e., if you modify this file, which other files might break).

- **`cis cycles`**
  Detects and lists circular dependencies within the project architecture.

### 3. Execution & Validation

- **`cis run <command>`**
  Executes a terminal command in a secure, sandboxed environment, capturing stdout and stderr for diagnostics tracking.

### 4. The MCP Server Mode

- **`cis mcp`**
  Starts the standard input/output (stdio) JSON-RPC server used exclusively by AI agents to communicate via the Model Context Protocol.

---

> **Note:** For all database commands to execute quickly and bypass Windows file locks, ensure you are running `cis` inside an initialized directory, and `cis scan` has been run at least once to populate the SQLite index.
