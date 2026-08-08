# CodeIntel & Codebase Analyzer

An AI-Assisted Local Codebase Analyzer built to provide deep insights into your software projects. This tool offers both a rich terminal-based UI (Textual) and a modern web-based dashboard (React/Vite) to visualize dependencies, detect issues, evaluate risks, and chat with an AI assistant about your codebase.

## 1. Project Overview
CodeIntel is a comprehensive analysis tool designed to help developers understand codebases quickly. It scans local project directories, identifies languages, traces execution paths, maps dependencies, flags potential errors (like circular dependencies), and calculates complexity and risk scores per file. An integrated AI assistant (powered by NVIDIA NIM) allows developers to ask natural-language questions about their project's structure and issues.

## 2. Key Features
- **Project Scanning:** Recursive project scanning with language detection and complexity metrics.
- **Dependency Mapping:** Analyzes file imports to build a complete dependency graph and detect circular dependencies.
- **Risk Assessment Engine:** Assigns High, Medium, or Low risk scores based on dependency centrality, error frequency, and code complexity.
- **Dual User Interfaces:**
  - A responsive Web Dashboard (React + D3/Three.js).
  - A Terminal UI (Textual) for CLI-based workflows.
- **AI Code Assistant:** Built-in chat using NVIDIA NIM APIs to answer questions about the analyzed codebase with context-awareness.

## 3. Architecture
The system is built on a modular architecture:
- **Core Analyzer Engine (Python):** Contains the analysis pipeline which runs sequentially: `Scanner` -> `DependencyAnalyzer` -> `ErrorDetector` -> `ExecutionTracer` -> `RiskEngine`.
- **FastAPI Backend:** Exposes the analysis engine and AI chat via REST and Server-Sent Events (SSE) endpoints.
- **Frontend (Web):** A React application that consumes the FastAPI endpoints to render an interactive 3D background, dependency graphs, issue lists, and chat panels.
- **Frontend (CLI):** A Textual-based UI interacting directly with the core Python modules.

## 4. Tech Stack
- **Backend:** Python 3, FastAPI, Uvicorn, NetworkX, Matplotlib, OpenAI SDK (for NVIDIA NIM integration).
- **CLI UI:** Textual, Rich.
- **Web Frontend:** React 18, Vite, D3.js, Three.js, Recharts, Framer Motion.

## 5. Project Structure
```text
.
├── codebase_analyzer/          # Python core analysis engine & CLI UI
│   ├── ai/                     # AI clients, prompt engines, context builder
│   ├── backend/                # Scanners, dependency/risk analyzers, tracers
│   ├── ui/                     # Textual panels for the terminal interface
│   ├── main.py                 # Entry point for the CLI Textual UI
│   ├── server.py               # FastAPI server backend
│   └── requirements.txt        # Python dependencies
└── codeintel_ui/               # React Web Interface
    ├── src/                    # React components, hooks, styles (App.jsx, etc.)
    ├── package.json            # Node.js dependencies
    └── vite.config.js          # Vite configuration
```

## 6. Installation/Setup

### Prerequisites
- Node.js (for web frontend)
- Python 3.10+ (for backend and CLI)

### Backend Setup
1. Navigate to the Python analyzer directory:
   ```bash
   cd codebase_analyzer
   ```
2. Install the dependencies:
   ```bash
   pip install -r requirements.txt
   ```
3. Install additional optional dependencies for the web server and graph rendering:
   ```bash
   pip install fastapi uvicorn networkx matplotlib openai
   ```

### Frontend Setup
1. Navigate to the React UI directory:
   ```bash
   cd codeintel_ui
   ```
2. Install Node dependencies:
   ```bash
   npm install
   ```

## 7. Environment Variables
- `NVIDIA_API_KEY`: Required on the backend server to enable the AI Chat capabilities. It is used to authenticate with the NVIDIA NIM API. You can add it to your environment or an `.env` file where you run the server.

## 8. How to Run the Project

### To run the React Web Dashboard:
1. Start the FastAPI backend:
   ```bash
   cd codebase_analyzer
   uvicorn server:app --reload --port 8000
   ```
2. Start the React frontend (in a new terminal):
   ```bash
   cd codeintel_ui
   npm run dev
   ```
3. Open the provided `localhost` URL from Vite in your browser.

### To run the Terminal CLI UI:
You can run the Textual UI directly without starting a web server:
```bash
cd codebase_analyzer
python main.py --path /path/to/your/project
```
You can also run `python main.py --demo` to see the UI with demo data.

## 9. API/Endpoints
The FastAPI server exposes the following main endpoints (running on port 8000):
- `GET /health` : Returns backend status and feature availability (AI/Graph setup).
- `POST /analyze` : Accepts `{ "path": "/local/path" }` and returns a comprehensive JSON object containing files, dependencies, issues, complexity, and risk data.
- `POST /chat` : Server-Sent Events (SSE) endpoint for streaming AI chat responses. Requires the `NVIDIA_API_KEY` to be set on the server.
- `POST /impact` : Returns dependents and dependencies for a specific file to analyze change impact.

## 10. Usage Examples
- **Analyzing a local directory:** In the React Web UI, type the absolute path to your project (e.g., `/home/user/projects/my-app`) in the top navigation bar and click "Analyze".
- **Finding high-risk code:** After analysis, click the "Complexity" or "Overview" tab to view files marked as High Risk due to high lines of code or complex branching.
- **Chatting with AI:** Go to the "AI Chat" tab and click "What are the entry points?" or ask custom questions like "Explain the circular dependency found in auth.py".

## 11. Current Limitations
- **Local Paths Only:** The application requires access to the file system to analyze code. It does not currently support cloning and analyzing remote Git repositories directly.
- **Language Support:** Complexity and AST parsing are optimized for Python, JS, TS, Java, C/C++, and Go. Other languages might have basic scanning but lack deep insights.
- **Scalability:** Analyzing extremely large monorepos (10,000+ files) might take significant time and consume heavy memory.

## 12. Future Improvements
- **Remote Repository Support:** Integration with GitHub/GitLab APIs to analyze remote repositories seamlessly.
- **Dockerization:** Provide `Dockerfile` and `docker-compose.yml` for effortless one-click setup of both frontend and backend.
- **Authentication:** Add user authentication and history saving for web UI sessions.
- **Deeper Semantic Analysis:** Use Tree-sitter for more robust cross-language AST parsing and control flow graphs.
- **Inline Fix Suggestions:** Have the AI not only explain issues but also generate inline patches to resolve them.
