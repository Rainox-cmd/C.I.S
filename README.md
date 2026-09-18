# C.I.S. — Complete Build & Architecture README

**Project:** C.I.S. (Context & Intelligence System)\
**Status:** Open / evolving design\
**Primary implementation direction:** Rust\
**Core principle:** Zero-AI core, external-AI interoperability

> This is a living specification, not a frozen contract. Features may be
> added, removed, simplified, redesigned, or postponed after
> implementation, testing, benchmarking, security review, or research.

## Current Decision Update

The current implementation priority is **local-first**.

- Project context lives inside the project's `.cis/` directory.
- Local project/session memory is the primary storage mechanism.
- Conversation archives, where available, are kept separately from canonical project memory.
- External AI is optional and connects to C.I.S. rather than becoming part of the core.
- Cloud storage is **not required for the initial build**.
- Google Drive, OneDrive, and other cloud providers remain future optional integrations.
- The design remains open to additional features after testing and research.

## 📖 Quick Start & Documentation

Want to get your AI agent connected to C.I.S., or use the commands manually?  
👉 **[Read the MCP Setup & CLI Guide here!](docs/mcp-guide.md)**

---

## 1. Vision

C.I.S. is a **local-first project intelligence and execution layer**. It
should work independently and optionally connect to external AI systems.

C.I.S. is **not another chatbot**.

### C.I.S. provides

-   Project structure and code intelligence
-   File and symbol relationships
-   Search and dependency information
-   Project and session context
-   Persistent memory
-   Git information and change tracking
-   Terminal execution and diagnostics
-   Controlled file operations
-   Visualization
-   CLI
-   MCP
-   Optional future cloud/context storage

### External AI provides

-   Reasoning
-   Planning
-   Natural-language understanding
-   Code generation/modification
-   Optional semantic/RAG capabilities
-   Higher-level decisions

The core must remain useful without AI.

## 2. Non-Negotiable Principles

### Zero-AI Core

C.I.S. must not require an embedded LLM, chatbot, embedding model,
vector database, AI API key, specific AI provider, or cloud AI service.

### Internet Is Optional

Core features should work offline. Internet may optionally be used for
GitHub, GitLab, remote Git, documentation, cloud storage, updates,
synchronization, and APIs.

### External AI Is Replaceable

C.I.S. must not depend on one AI provider. Users should be able to
switch external AI tools without losing project context.

### Project Isolation

Context from one project must never silently leak into another.

### Deterministic First

Preferred retrieval:

``` text
Project scope
→ Metadata
→ FTS5
→ Symbols
→ Dependencies
→ Git/change information
→ Ranking
→ Context budget
```

Embeddings/RAG are optional future external-AI enhancements, not core
requirements.

### Incremental Processing

After initial indexing, changed files should be reprocessed instead of
rescanning the entire project.

### Security From the Beginning

Terminal/file execution must have permission controls, path containment,
command restrictions, auditing, and secret protection.

## 3. Operating Modes

### Mode A: Fully Local / Offline

Should support:

-   Project scanning
-   Language detection
-   Code analysis
-   Dependency information
-   Search
-   Git tracking
-   Metadata
-   Context and memory
-   Controlled terminal commands
-   Error capture
-   Graphs
-   CLI/UI
-   Context export/import
-   Local MCP

### Mode B: C.I.S. + External AI

External AI connects through MCP/API/CLI. C.I.S. provides project
intelligence, memory, context, terminal results, errors, metadata,
history, and relevant code.

### Mode C: C.I.S. + External AI + Optional Cloud

Adds cloud context storage, backup, synchronization, and other optional
services.

## 4. Technology Stack

### Core

**Rust**

Reasons:

-   Performance
-   Low memory overhead
-   Filesystem/process tooling
-   CLI ecosystem
-   Concurrency
-   Native executable distribution
-   Good fit for a local developer tool

### CLI

Use `clap`.

Initial command ideas:

``` text
cis init
cis scan
cis analyze
cis status
cis doctor
cis search "authentication"
cis symbol UserService
cis deps path/to/file
cis impact path/to/file
cis issues
cis changes
cis context
cis memory
cis context export
cis context import
cis git status
cis git diff
cis terminal
cis mcp
```

Support machine-readable output such as:

``` text
cis analyze --json
cis search "authentication" --json
cis issues --json
cis status --json
```

### Database

**SQLite**, with one project-local database:

``` text
.cis/project.db
```

### Search

**SQLite FTS5** for deterministic full-text search.

### Code Parsing

**Tree-sitter**

Initial deep-analysis languages:

-   Python
-   JavaScript
-   TypeScript
-   Rust
-   Go

Expand progressively. Unsupported languages should still receive file
metadata, Git tracking, and text search where possible.

### Structural Search

**ast-grep** may be added later. It is useful but not required for the
first MVP.

### Git

Start with the normal **Git CLI** for reliability and compatibility.

Consider **gitoxide/gix** later for in-process reads if benchmarks
justify it. Do not add it only for theoretical performance.

### MCP

Use MCP as the main external-AI integration mechanism. Expose
coarse-grained, task-oriented tools rather than forcing many tiny calls.

### Terminal

Use Rust process management with:

-   Working-directory restrictions
-   Path containment
-   Command policies
-   Confirmation for risky commands
-   Timeouts
-   Restricted environment
-   Output limits
-   Audit logs
-   Secret redaction where practical

### UI

Potential terminal UI:

-   `ratatui`
-   `crossterm`

A graphical/web UI can follow later.

### Cloud

**Deferred and optional.**

Cloud storage is not part of the early core. The initial system must rely on
project-local `.cis/` storage and local context export/import. Google Drive,
OneDrive, S3, or other providers may be added later through an adapter without
becoming a core dependency.

Use an abstraction:

``` text
StorageProvider
├── LocalStorage
└── GoogleDriveStorage
```

Future providers can be added later.

## 5. Repository Architecture

Start simple:

``` text
cis/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── docs/
├── tests/
└── src/
    ├── main.rs
    ├── cli/
    ├── config/
    ├── scanner/
    ├── language/
    ├── parser/
    ├── index/
    ├── graph/
    ├── project/
    ├── git/
    ├── memory/
    ├── context/
    ├── terminal/
    ├── security/
    ├── mcp/
    └── diagnostics/
```

Only extract separate Cargo workspace crates when boundaries are proven:

``` text
crates/
├── cis-core
├── cis-scanner
├── cis-parser
├── cis-index
├── cis-memory
├── cis-git
├── cis-terminal
├── cis-mcp
└── cis-cli
```

Do not over-split the project early.

## 6. `.cis` Project Storage

Each project should have an isolated directory:

``` text
project/
└── .cis/
    ├── project.db
    ├── config.toml
    ├── storage/
    │   ├── project/
    │   └── sessions/
    ├── exports/
    └── logs/
```

### Project memory

Durable information:

-   Architecture decisions
-   Project rules
-   Framework conventions
-   Important constraints
-   Confirmed relationships
-   Developer-confirmed labels

### Session memory

Temporary information:

-   Current task
-   Debugging history
-   Recent errors
-   Failed approaches
-   Recent diffs
-   Temporary discoveries

Session memory must be prunable.

## 7. File Labels

Do **not** rename files simply to make them easier for AI.

Store metadata instead:

``` text
auth/service.py
role = authentication
component = backend
importance = high
```

Possible metadata:

-   Role
-   Component
-   Language
-   Purpose
-   Entry-point status
-   Test status
-   Generated status
-   Dependency role
-   User-confirmed description
-   AI-generated description
-   Detection source
-   Last verification

Provenance should distinguish:

``` text
detected
ai_generated
developer_confirmed
imported
```

## 8. Language Detection and Project Roots

Language detection should use deterministic signals:

-   Extensions
-   Shebangs
-   Known filenames
-   Project configuration
-   Build files
-   Package manifests

C.I.S. should distinguish **language detection** from **deep structural
analysis**.

It should not assume every project has one root file.

Use:

-   Project manifests
-   Build configuration
-   Package configuration
-   Entry-point conventions
-   Import/dependency relationships
-   Executable definitions
-   Framework configuration

Large projects may contain several applications:

``` text
Project
├── Backend
├── Frontend
├── Worker
└── Shared
```

If uncertain, report candidate entry points with confidence rather than
pretending certainty.

## 9. Project Graph

Support several relationship types.

### Hierarchy

``` text
Project
├── Backend
├── Frontend
├── Tests
└── Infrastructure
```

### File dependencies

``` text
main.py
  ↓
api.py
  ├── auth.py
  └── database.py
```

### Symbols

``` text
AuthManager
├── login()
├── logout()
└── validate_token()
```

### Tests

``` text
auth.py
↑
test_auth.py
```

### Git

``` text
auth.py
↓
commit abc123
```

### Context

``` text
Current task
├── auth.py
├── database.py
├── AuthManager
└── relevant project memory
```

The UI must use lazy loading, filtering, grouping, and hierarchy for
large graphs.

## 10. Project Scale

Do not impose an arbitrary hard file limit.

Practical engineering targets:

  Size               Target
  ------------------ -------------------------------
  \<5,000 files      Easy
  5,000--25,000      Comfortable
  25,000--100,000    Incremental indexing required
  100,000--500,000   Benchmark/optimize
  500,000+           Large-monorepo territory

These are targets, not guarantees.

Ignore common dependency/generated directories where appropriate:

``` text
.git/
node_modules/
.venv/
venv/
__pycache__/
dist/
build/
target/
generated/
```

Respect `.gitignore` and `.cisignore`.

## 11. Performance

Initial analysis may roughly be:

-   Small projects: seconds
-   Medium projects: tens of seconds to minutes
-   Large projects: minutes or more

These must be benchmarked, not assumed.

Incremental indexing is essential:

``` text
Initial:
10,000 files
→ full index

Later:
auth.py changed
→ detect hash/metadata change
→ reparse auth.py
→ update affected data
```

Background indexing must minimize CPU, RAM, and disk impact.

## 12. Deterministic Context Retrieval

Recommended pipeline:

``` text
Request
→ Project/session scope
→ Metadata filtering
→ FTS5
→ Symbol search
→ Dependency/impact graph
→ Git relevance
→ Memory relevance
→ Ranking
→ Context budget
→ Structured response
```

The goal is the **smallest useful context**, not the largest possible
response.

## 13. Context Budgeting

External AI systems have different context limits, token limits, and
rate limits.

C.I.S. should let retrieval operate under configurable budgets.

Potential retrieval levels:

``` text
Level 0: Project metadata
Level 1: Relevant files
Level 2: Relevant symbols
Level 3: Dependencies/relationships
Level 4: Relevant source excerpts
Level 5: Git changes/diagnostics
Level 6: Relevant memory/history
```

Exact levels may change.

## 14. First-Time AI Understanding

Without C.I.S., an AI may repeatedly:

``` text
list files
→ read README
→ search
→ inspect imports
→ read files
→ inspect Git
→ run commands
→ repeat
```

With C.I.S.:

``` text
AI
→ project overview
→ structured project map
→ task-specific context
```

C.I.S. should expose coarse-grained MCP tools such as:

``` text
get_project_overview
search_project
get_file_context
get_symbol
get_dependencies
get_impact
get_project_memory
get_session_context
get_recent_changes
get_diagnostics
run_command
```

Exact names are not final.

## 15. Memory and Context Lifecycle

Project memory and session memory must remain separate.

When a context/session limit is reached:

1.  Warn the user.
2.  Show what would be removed.
3.  Offer export/archive.
4.  Allow saving elsewhere.
5.  Delete only according to configured policy.

Never silently delete valuable context.

Portable context should support:

``` text
cis context export
cis context import
```

An imported archive should check:

-   Project path
-   File existence
-   Hashes where available
-   Stale references
-   Changed files

## 16. Google Drive

Google Drive is the initial cloud candidate.

Cloud is optional and must not become a core dependency.

Use:

``` text
StorageProvider
├── Local
└── Google Drive
```

Cloud failure must not break local C.I.S.

Encryption before cloud upload should be evaluated before storing
sensitive context.

## 17. Git

Git should be core.

Support:

-   Branch
-   Status
-   Uncommitted changes
-   Diff
-   Commit history
-   File history
-   Blame when useful
-   Recent changes

GitHub should remain an optional adapter.

## 18. Terminal and Diagnostics

Execution pipeline:

``` text
Request
→ Permission check
→ Working-directory check
→ Environment restrictions
→ Timeout/resource limits
→ Execute
→ Capture stdout/stderr
→ Parse diagnostics
→ Audit
```

Security minimum:

-   Path containment
-   Command policy
-   Explicit confirmation for risky operations
-   Restricted environment
-   Timeouts
-   Output limits
-   Audit logs
-   Secret protection

Never begin with unrestricted AI shell access.

Diagnostics should capture:

-   stdout
-   stderr
-   exit code
-   stack traces
-   file paths
-   line numbers
-   process state

Connect diagnostics to files, symbols, Git changes, sessions, and AI
context.

## 19. MCP

MCP should expose task-oriented tools, not dozens of tiny operations.

Possible tools:

``` text
project_overview
search
file_context
symbol_context
dependency_context
impact_analysis
memory_context
session_context
git_context
diagnostics
run_command
```

MCP must be an adapter layer. Core business logic must not depend on
MCP.

## 20. UI

The UI should visualize deterministic core data.

Views:

-   Project tree
-   Dependency graph
-   Symbol graph
-   Impact analysis
-   Git changes
-   Context selected for AI
-   Memory
-   Terminal/processes
-   Diagnostics

The context view should answer:

``` text
Why was this file selected?
Why was this memory selected?
Why was this context returned?
```

## 21. Phased Roadmap

### Phase 0 --- Audit and Bug Fixing

Before major features:

-   Inspect existing architecture
-   Run existing system
-   Reproduce bugs
-   Document current behavior
-   Review dependencies
-   Remove/replace AI dependencies that violate zero-AI core
-   Review filesystem/path handling
-   Review configuration
-   Review tests
-   Review security
-   Review Windows compatibility
-   Add regression tests

**Deliverable:** stable baseline and migration plan.

### Phase 1 --- Rust Foundation

Build:

-   Cargo project
-   CLI
-   Configuration
-   Logging
-   Error handling
-   `.cis`
-   SQLite
-   `cis init`
-   `cis doctor`
-   `cis status`

Acceptance:

-   Builds
-   Runs on Windows
-   Creates `.cis`
-   SQLite works
-   Tests run

### Phase 2 --- Scanner

Build:

-   Directory scanning
-   Ignore rules
-   File metadata
-   Hashing
-   Language detection
-   File classification
-   Project boundaries

### Phase 3 --- Tree-sitter Intelligence

Start with:

-   Python
-   JavaScript
-   TypeScript
-   Rust
-   Go

Extract:

-   Functions
-   Classes
-   Methods
-   Imports
-   Exports
-   Interfaces where supported
-   Modules
-   Symbol locations

### Phase 4 --- SQLite + FTS5

Build:

-   Schema
-   File table
-   Symbol table
-   Relationship table
-   Metadata
-   FTS5
-   Index updates

Commands:

``` text
cis search
cis symbol
cis deps
```

### Phase 5 --- Graph and Impact

Build:

-   File graph
-   Symbol graph
-   Import graph
-   Dependency graph
-   Entry-point detection
-   Component grouping
-   Impact analysis

### Phase 6 --- Incremental Indexing

Build:

-   Hash checks
-   Incremental parsing
-   Incremental index updates
-   Caching
-   Background work
-   Large-project tests
-   Resource controls

Benchmark:

-   500 files
-   5,000 files
-   25,000 files
-   100,000 files if hardware permits

Measure:

-   Scan time
-   Incremental update time
-   RAM
-   CPU
-   DB size
-   Search latency

### Phase 7 --- Git

Add:

-   Status
-   Diff
-   Log
-   File history
-   Recent changes

Connect Git data to files/symbols.

### Phase 8 --- Context and Memory

Build:

-   Project memory
-   Session memory
-   Provenance
-   Tags
-   TTL/pruning
-   Retrieval
-   Context budgets
-   Project isolation

### Phase 9 --- Terminal and Diagnostics

Build:

-   Command runner
-   Path restrictions
-   Allowlist/policy
-   Confirmation
-   Timeout
-   Output capture
-   Process management
-   Error parsing
-   Audit logs

Advanced OS sandboxing can follow once the basic security model is
stable.

### Phase 10 --- MCP

Build:

-   stdio MCP server
-   Tool definitions
-   Structured responses
-   Permission-aware execution
-   Context budgeting

Test with multiple external AI clients.

### Phase 11 --- Context Export/Import

Build:

``` text
cis context export
cis context import
```

Detect stale references and changed files.

### Phase 12 --- UI

Build:

-   Project tree
-   Graph
-   Search
-   Symbols
-   Dependencies
-   Impact
-   Git
-   Memory
-   Context
-   Terminal
-   Diagnostics

The UI consumes core data. It does not become a second intelligence
engine.

### Phase 13 --- Google Drive

Build:

-   Authentication
-   StorageProvider
-   Upload
-   Download
-   Restore
-   Failure handling

### Phase 14 --- GitHub Adapter

Optional:

-   Repository metadata
-   Issues
-   Pull requests
-   Discussions
-   Issue-to-file relationships

### Phase 15 --- Release Engineering

Build:

-   Windows release
-   Linux release
-   macOS release
-   Versioning
-   Migrations
-   Documentation
-   Upgrade checks
-   Automated tests
-   Regression suite

## 22. Testing

### Unit tests

Cover:

-   Path handling
-   Language detection
-   Parsing
-   Hashing
-   SQLite
-   Retrieval
-   Permissions
-   Context limits

### Integration tests

Test:

``` text
scan → parse → index → search
```

``` text
modify → incremental update → search
```

``` text
terminal → diagnostics → context
```

``` text
memory → export → import
```

### Security tests

Test:

-   Path traversal
-   Dangerous commands
-   Environment leaks
-   Secret exposure
-   Unauthorized access
-   Symlinks/junctions
-   Windows-specific paths

## 23. Technical Risks

-   Cross-platform terminal security
-   Tree-sitter memory use on huge repositories
-   Index drift
-   Git behavior differences
-   Context explosion
-   MCP changes
-   Background CPU/disk consumption
-   False project assumptions

## 24. Product Risks

-   Too much background resource usage
-   Too much context returned
-   Overengineering
-   Security failures
-   Presenting uncertain information as fact

Track provenance:

``` text
detected
inferred
ai_generated
developer_confirmed
imported
```

## 25. Features Not Required for Core MVP

Do not add these without a demonstrated need:

-   Embedded chatbot
-   Embedded LLM
-   Mandatory embeddings
-   Mandatory vector database
-   Mandatory cloud
-   Mandatory GitHub
-   Docker
-   Kubernetes
-   Full autonomous AI
-   Distributed databases
-   Microservices
-   Global database
-   Automatic file renaming
-   Custom AST parser
-   Complex cloud synchronization

## 26. Future Feature Candidates

Not commitments:

-   More Git providers
-   More cloud providers
-   LSP
-   More languages
-   ast-grep
-   Dependency vulnerability scanning
-   Test coverage visualization
-   Architecture drift detection
-   Secret detection
-   Environment/config tracking
-   Build-system detection
-   CI/CD integration
-   Docker awareness
-   Container logs
-   Database schema visualization
-   API endpoint mapping
-   Documentation mapping
-   Project health score
-   Refactor impact analysis
-   Workspace snapshots
-   Context versioning
-   Context encryption
-   Plugin system
-   Advanced OS sandboxing
-   More AI clients
-   Remote C.I.S.

## 27. AI Token and Rate-Limit Strategy

C.I.S. cannot remove an external AI provider's limits.

It can reduce unnecessary calls.

The intended workflow is:

``` text
First connection
→ local deterministic index

AI
→ compact project overview

AI
→ task-specific context

C.I.S.
→ only relevant files/symbols/history/memory
```

Repeated sessions should not require the AI to rediscover the entire
project.

## 28. AI-Generated Memory and Labels

External AI may propose:

-   File descriptions
-   Labels
-   Architecture decisions
-   Project memory
-   Session notes

Store provenance.

Example:

``` text
memory:
"Authentication uses JWT."

source:
ai_generated

status:
unverified
```

After developer confirmation:

``` text
source:
developer_confirmed
```

AI-generated memory must not automatically become permanent truth.

## 29. MVP Definition

The first useful MVP should contain:

``` text
Rust executable
+
CLI
+
.cis
+
SQLite
+
FTS5
+
File scanner
+
Language detection
+
Tree-sitter
+
Basic symbols
+
Basic dependency graph
+
Incremental indexing
+
Git status/diff
+
Project/session memory
+
Deterministic context retrieval
+
Basic terminal permissions
+
MCP
```

UI, cloud, GitHub, advanced sandboxing, and additional languages can
follow.

## 30. MVP Success Workflow

The MVP should prove:

``` text
1. Open an existing project.
2. Run cis init.
3. Run cis scan.
4. Detect project structure.
5. Detect languages.
6. Identify entry-point candidates.
7. Parse symbols/imports.
8. Store the index in SQLite.
9. Search with FTS5.
10. Build relationships.
11. Modify a file.
12. Incrementally update affected data.
13. Run tests.
14. Capture errors.
15. Store project/session memory.
16. Connect an external AI through MCP.
17. Return compact relevant context.
18. Receive a terminal request.
19. Check permissions.
20. Execute and return structured output.
21. Let the user inspect selected context and affected files.
```

If this works reliably, the core concept is proven.

## 31. Development Rules for Kilo

1.  **Inspect before changing.** Read the existing repository, tests,
    dependencies, and configuration before rewriting.
2.  **Fix before expanding.** Stabilize current functionality first.
3.  **Do not add AI to the core.**
4.  **Keep modules/adapters separate.**
5.  **Avoid premature complexity.**
6.  **Security cannot be postponed for execution features.**
7.  **Benchmark before claiming performance improvements.**
8.  **Prefer deterministic local processing.**
9.  **Never silently delete user data.**
10. **Maintain migrations/backward compatibility where practical.**
11. **Do not assume every proposed future feature is mandatory.**
12. **Use the smallest reversible implementation when requirements are
    ambiguous.**
13. **Stop and report architectural conflicts instead of silently making
    major assumptions.**
14. **Keep external AI providers interchangeable.**
15. **Keep optional cloud/GitHub integrations outside the core.**

## 32. Definition of Done

A phase is complete only when it has:

``` text
Implementation
+
Unit tests
+
Integration tests
+
Error handling
+
Documentation
+
CLI verification
+
Performance check where relevant
+
Security review where relevant
+
Regression test
```

## 33. Research Strategy

Research should continue, but it should be targeted.

Use:

``` text
Research
→ implement
→ test
→ discover problem
→ research that problem
→ improve
```

High-value topics:

1.  Tree-sitter cross-language normalization
2.  Rust + SQLite architecture
3.  Incremental indexing
4.  FTS5 ranking
5.  Dependency graphs
6.  MCP tool design
7.  Windows process security
8.  Terminal restrictions
9.  Secret redaction
10. Context budgeting
11. Memory provenance
12. Google Drive authentication/storage
13. Git vs gitoxide
14. Large-repository benchmarks
15. Graph UI scalability

Do not perform endless broad architecture research after the core
direction is sufficiently understood.

## 34. Final Architecture

``` text
                         External AI
                  Claude / Gemini / other AI
                              |
                       MCP / CLI / API
                              |
                              v
                    +-------------------+
                    |      C.I.S.       |
                    |    Rust Core      |
                    +-------------------+
                       |    |    |    |
             +---------+    |    |    +---------+
             |              |    |              |
          Scanner        Git  Terminal      Security
             |
        Tree-sitter
             |
       Project Intelligence
             |
     +-------+--------+
     |                |
  Symbols         Dependencies
     |                |
     +-------+--------+
             |
          SQLite
           + FTS5
             |
      Retrieval Engine
             |
       Context Budget
             |
       +-----+------+
       |            |
 Project Memory  Session Memory
       |            |
       +-----+------+
             |
        Export/Import
             |
      Storage Provider
        /                Local       Google Drive
```

## 35. Product Identity

**C.I.S. is a local-first project intelligence and execution layer that
gives developers and external AI agents structured access to codebases,
persistent project context, terminal execution, diagnostics, Git
history, change tracking, and project metadata through CLI, MCP, and
other interfaces.**

The differentiator is not:

> "C.I.S. is another AI coding assistant."

It is:

> **C.I.S. is the persistent project layer that AI coding tools can
> use.**

## 36. Current Decision Table

  Area                       Current Direction               Status
  -------------------------- ------------------------------- ----------------------------------
  Built-in AI                No                              Strong direction
  External AI                Yes, optional                   Strong direction
  Internet                   Allowed                         Strong direction
  Offline core               Yes                             Strong direction
  Rust                       Yes                             Current implementation direction
  CLI                        Yes                             Planned
  Terminal                   Yes                             Planned
  MCP                        Yes                             Planned
  Project memory             Yes                             Planned
  Session memory             Yes                             Planned
  Local context              Yes                             Planned
  Cloud context              Optional                        Proposed
  Google Drive               Initial candidate               Proposed
  RAG                        Optional future enhancement     Not final
  Embeddings                 External/optional               Not final
  Git                        Yes                             Strong direction
  GitHub                     Optional adapter                Proposed
  File labels                Metadata, not forced renaming   Proposed
  UI graphs                  Yes                             Planned
  Context export/import      Yes                             Proposed
  Context deletion warning   Yes                             Proposed
  Automatic deletion         Carefully controlled            Not final
  Context limits             Yes, configurable               Proposed
  Security/permissions       Required                        Strong direction
  Vector database            Not required                    Open/future
  Exact roadmap              Phased                          Strong direction

## 37. Instruction to the Implementing AI

You are implementing an evolving project called **C.I.S. (Context &
Intelligence System)**.

Do not treat every item in this document as a requirement for the
current phase.

Your responsibilities:

-   Inspect the existing repository first.
-   Identify current architecture and bugs.
-   Preserve useful existing functionality where practical.
-   Implement phase by phase.
-   Keep C.I.S. AI-free internally.
-   Use external AI only through optional interfaces.
-   Prefer deterministic local processing.
-   Keep project context isolated.
-   Never silently delete user data.
-   Treat terminal execution as security-sensitive.
-   Test every meaningful subsystem.
-   Keep dependencies minimal.
-   Do not introduce Docker/Kubernetes/cloud infrastructure without a
    genuine requirement.
-   Benchmark before replacing working components for performance.
-   Stop and report architectural conflicts rather than making large
    silent assumptions.
-   Use the smallest reversible implementation when requirements are
    ambiguous.
-   Keep AI providers, Git providers, cloud providers, languages, and UI
    implementations modular.

### Primary objective

Build a reliable, local-first, zero-AI project intelligence layer that
makes external AI systems dramatically more efficient at understanding
and working with real software projects.

**Do not build another chatbot. Build the infrastructure that lets AI
understand the project without repeatedly rediscovering it from
scratch.**
