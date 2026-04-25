# 🧠 AI-Assisted Local Codebase Analyzer
### Multi-Language Code Intelligence System with NVIDIA NIM Chat

A fully offline Python desktop tool that analyzes any code project folder,
detects issues, visualizes dependencies, and answers questions about your
code using AI — all from your terminal.

---

## 📁 Project Structure

```
codebase_analyzer/
│
├── main.py                        ← Entry point (run this)
├── requirements.txt               ← All pip dependencies
│
├── backend/                       ← Core analysis engine
│   ├── scanner.py                 ← Recursive file scanner
│   ├── dependency_analyzer.py     ← AST + regex import parser, graph builder
│   ├── error_detector.py          ← Static issue detection
│   ├── execution_tracer.py        ← Safe Python execution + failure chains
│   ├── risk_engine.py             ← Multi-factor risk scoring (Low/Med/High)
│   └── graph_renderer.py          ← NetworkX + Matplotlib graph PNG renderer
│
├── ai/                            ← AI integration (optional)
│   ├── nim_client.py              ← NVIDIA NIM API streaming client
│   ├── context_builder.py         ← Builds focused project context per query
│   └── prompt_engine.py           ← Formats prompts, parses confidence scores
│
├── ui/                            ← Textual terminal UI
│   ├── main_window.py             ← Main app — assembles all panels, routes messages
│   └── panels/
│       ├── file_explorer.py       ← Left sidebar: project file tree
│       ├── dashboard.py           ← Top center: metrics and language distribution
│       ├── graph_panel.py         ← Graph view + node detail sidebar
│       ├── error_console.py       ← Error and trace log with filters
│       └── chat_panel.py          ← AI chat with live streaming responses
│
└── utils/
    ├── logger.py                  ← Centralized logging (writes to ~/.codebase_analyzer/logs/)
    └── file_handler.py            ← ZIP extraction, path normalization
```

---

## ⚙️ Requirements

### Python Version
```
Python 3.11 or higher  (3.12 recommended)
```

### Step 1 — Install all required libraries

Run this single command to install everything:

```bash
pip install textual rich networkx matplotlib openai
```

Or install from the included requirements file:

```bash
pip install -r requirements.txt
```

### Full library list with versions

| Library | Version | Purpose |
|---|---|---|
| `textual` | ≥ 0.47.0 | Terminal UI framework (panels, layout, widgets) |
| `rich` | ≥ 13.7.0 | Text styling, tables, markdown inside the terminal |
| `networkx` | ≥ 3.2 | Dependency graph construction, cycle detection, centrality |
| `matplotlib` | ≥ 3.8 | Renders the dependency graph as a PNG image |
| `openai` | ≥ 1.12.0 | NVIDIA NIM uses OpenAI-compatible API (only needed for AI chat) |

### What is already built-in (no install needed)
```
ast          → Python code parsing
subprocess   → Safe execution checks
zipfile      → ZIP file extraction
asyncio      → Async workers (UI never freezes)
argparse     → CLI mode
difflib      → Similarity detection
pathlib      → Path handling
logging      → Log file output
```

---

## 🚀 How to Run

### Option 1 — Full Textual UI (recommended)

```bash
cd codebase_analyzer
python main.py
```

This opens the interactive terminal dashboard. Then:
1. Type your project folder path in the left panel input box
2. Click **▶ Analyze** or press Enter
3. Wait for all 5 analysis stages to complete (~2–10 seconds)
4. Browse results across the 5 tabs

### Option 2 — Analyze a specific project directly

```bash
python main.py --path /path/to/your/project
```

### Option 3 — Analyze a ZIP file

```bash
python main.py --path /path/to/project.zip
```
The ZIP is extracted safely to a temp folder automatically.

### Option 4 — CLI mode (no UI, terminal output only)

```bash
python main.py --path /path/to/project
```

### Option 5 — With AI chat enabled

```bash
# Set your NVIDIA API key first
export NVIDIA_API_KEY="nvapi-xxxxxxxxxxxxxxxxxxxx"

# Then run normally
python main.py
```

On Windows:
```cmd
set NVIDIA_API_KEY=nvapi-xxxxxxxxxxxxxxxxxxxx
python main.py
```

---

## 🤖 AI Chat Setup (Optional)

The AI chat feature uses **NVIDIA NIM** — a free API for running Llama 3.1 70B.

### Step 1 — Get a free NVIDIA API key
1. Go to: https://build.nvidia.com
2. Sign up for a free account
3. Navigate to any model page (e.g. Llama 3.1)
4. Click **Get API Key**
5. Copy the key (starts with `nvapi-`)

### Step 2 — Set the environment variable

**Linux / macOS:**
```bash
export NVIDIA_API_KEY="nvapi-your-key-here"
```

To make it permanent, add that line to your `~/.bashrc` or `~/.zshrc`.

**Windows (Command Prompt):**
```cmd
set NVIDIA_API_KEY=nvapi-your-key-here
```

**Windows (PowerShell):**
```powershell
$env:NVIDIA_API_KEY="nvapi-your-key-here"
```

### Step 3 — Run the app
```bash
python main.py
```

The app will automatically:
- Detect the API key on startup
- Connect to NVIDIA NIM
- Show ✔ Connected in the chat panel
- Enable the chat panel for questions

### AI Models used
- **Primary:** `meta/llama-3.1-70b-instruct` (high quality)
- **Fallback:** `microsoft/phi-3.5-mini-instruct` (if primary fails)

### What you can ask the AI
After analyzing a project, type questions like:
- *"What are the highest-risk files?"*
- *"Explain the circular dependency"*
- *"Which file would cause the most failures if it broke?"*
- *"What errors were detected and why?"*
- *"Which files are entry points?"*
- *"Why is engine.py high risk?"*

The AI only uses context from your actual project — it never hallucinates file names or issues.

---

## 🖥️ UI Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│  ⬡ CODE.INTEL          [Header + clock]                                 │
├──────────────┬──────────────────────────────────────┬───────────────────┤
│              │                                      │                   │
│  📁 FILE     │        📊 DASHBOARD                  │  🤖 AI CHAT       │
│  EXPLORER    │   (metrics, language %, issues)      │                   │
│              │                                      │  (streaming       │
│  [path input]│                                      │   responses)      │
│  [▶ Analyze] │                                      │                   │
│              ├──────────────────────────────────────│                   │
│  file list   │  🕸 GRAPH    │  ⛔ ERRORS            │                   │
│  with risk   │  (dep graph  │  (error log,          │                   │
│  indicators  │  + node info)│   traces, filters)    │                   │
│              │              │                       │                   │
└──────────────┴──────────────────────────────────────┴───────────────────┘
│  [Footer: keyboard shortcuts]                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `Ctrl+A` | Focus path input |
| `Ctrl+C` | Focus chat input |
| `Ctrl+G` | Switch to Graph tab |
| `Ctrl+E` | Switch to Error Console tab |
| `Ctrl+D` | Toggle chat sidebar |
| `Ctrl+P` | Load demo data (no real project needed) |
| `F1` | Show help in error console |
| `Ctrl+Q` | Quit |

---

## 📊 What Gets Analyzed

| Feature | Languages |
|---|---|
| Import/dependency extraction | Python (AST), JavaScript, TypeScript, Java |
| Syntax error detection | Python (AST parse), JS, Java |
| Function extraction | Python, JavaScript, Java |
| Complexity scoring | All languages (keyword counting) |
| Safe execution check | Python only (compile + subprocess, 3s timeout) |
| Risk scoring | All files (centrality + errors + complexity) |
| Circular dependency detection | All languages |
| Entry point detection | Python, JavaScript |

---

## 📄 Output Files

After each analysis, the tool creates:

| Output | Location | Description |
|---|---|---|
| Graph PNG | System temp folder | Dependency graph image (open in any image viewer) |
| Log file | `~/.codebase_analyzer/logs/` | Full debug log for each run |

The graph PNG path is shown in the Error Console after rendering.
Open it in any image viewer (Windows Photos, Preview on Mac, eog on Linux).

---

## 🔒 Safety

- **No code is freely executed.** Python files only go through `compile()` and `py_compile` in a subprocess with a 3-second hard timeout.
- **No network calls** are made during analysis (only AI chat uses the network).
- **No file system writes** happen to your project folder. All output goes to temp folders.
- ZIP files are extracted with path-traversal protection (zip-slip guard).

---

## ❗ Troubleshooting

### `ModuleNotFoundError: No module named 'textual'`
```bash
pip install textual rich
```

### `ModuleNotFoundError: No module named 'networkx'`
```bash
pip install networkx matplotlib
```

### AI chat says "not connected"
- Check that `NVIDIA_API_KEY` is set in your terminal session
- Verify the key starts with `nvapi-`
- Make sure you have internet access for the API call

### Graph image not showing inside terminal
This is expected — the terminal cannot display PNG images inline.
The PNG path is printed in the Error Console. Open it in your system image viewer.

### `SyntaxError` in your own project files
This is intentional — the tool detects and reports them. Check the Error Console tab.

### App is slow on very large projects (1000+ files)
Use a more specific subfolder as the input path, e.g. `/project/src` instead of the full repo root.

---

## 🧪 Testing the Tool

To verify everything works before analyzing your real project:

```bash
# Press Ctrl+P inside the UI to load built-in demo data
# OR run:
python main.py --demo
```

This loads a fake 12-file project instantly with pre-populated issues, graph, and risk scores.

---

## 📦 requirements.txt contents

```
textual>=0.47.0
rich>=13.7.0
networkx>=3.2
matplotlib>=3.8
openai>=1.12.0
```
