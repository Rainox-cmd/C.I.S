"""
server.py  —  FastAPI bridge for the CodeIntel React UI
Place this file inside:  codebase_analyzer_final/codebase_analyzer/server.py
Run from that folder:    uvicorn server:app --reload --port 8000
"""
from dotenv import load_dotenv
load_dotenv()
import os
import sys
import json
import time
import tempfile
import asyncio
from pathlib import Path

# ── Make sure the project root is on sys.path ─────────────────────────────────
ROOT = Path(__file__).resolve().parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse, JSONResponse
from pydantic import BaseModel

# ── Backend imports ───────────────────────────────────────────────────────────
from backend.scanner            import ProjectScanner
from backend.dependency_analyzer import DependencyAnalyzer
from backend.error_detector     import ErrorDetector
from backend.execution_tracer   import ExecutionTracer
from backend.risk_engine        import RiskEngine
from utils.logger               import get_logger

log = get_logger(__name__)

# ── Try optional graph renderer ───────────────────────────────────────────────
try:
    from backend.graph_renderer import GraphRenderer
    HAS_GRAPH = True
except ImportError:
    HAS_GRAPH = False
    log.warning("graph_renderer unavailable — pip install networkx matplotlib")

# ── Try AI modules ────────────────────────────────────────────────────────────
try:
    from ai.nim_client    import NIMClient
    from ai.context_builder import ContextBuilder
    from ai.prompt_engine import PromptEngine
    HAS_AI = True
except ImportError:
    HAS_AI = False
    log.warning("AI modules unavailable — pip install openai")


# ── App ───────────────────────────────────────────────────────────────────────
app = FastAPI(title="CodeIntel API", version="2.0.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# Store last analysis results in memory for AI context
_last_results: dict = {}


# ── Request schemas ───────────────────────────────────────────────────────────

class AnalyzeRequest(BaseModel):
    path:              str
    safe_exec:         bool = False
    detect_duplicates: bool = False


class ChatRequest(BaseModel):
    question:     str
    context_file: str | None = None
    api_key:      str | None = None
    history:      list[dict] = []


# ── Helpers ───────────────────────────────────────────────────────────────────

def _run_pipeline(path: str, safe_exec: bool = False) -> dict:
    """
    Full analysis pipeline.
    Returns a dict matching what the React UI expects.
    """
    global _last_results

    resolved = Path(path).expanduser().resolve()
    if not resolved.exists() or not resolved.is_dir():
        raise ValueError(f"Not a valid directory: {resolved}")

    t0 = time.perf_counter()

    # Stage 1: Scan
    scan = ProjectScanner(resolved).scan()

    # Stage 2: Dependencies
    dep = DependencyAnalyzer(scan).analyze()

    # Stage 3: Errors
    errors = ErrorDetector(scan, dep).detect()

    # Stage 4: Execution trace
    trace = ExecutionTracer(scan, dep).trace()

    # Stage 5: Risk
    risk = RiskEngine(scan, dep, errors, trace).score()

    elapsed = round(time.perf_counter() - t0, 2)

    # Store for AI context
    _last_results = {
        "scan":   scan,
        "dep":    dep,
        "errors": errors,
        "trace":  trace,
        "risk":   risk,
    }

    # Build summary
    counts = risk.counts()
    issue_counts = errors.count_by_severity()

    summary = {
        "total_files":              len(scan.files),
        "total_issues":             len(errors.issues),
        "languages":                scan.language_counts,
        "language_distribution":    scan.language_dist,
        "high_risk":                counts["high"],
        "medium_risk":              counts["medium"],
        "low_risk":                 counts["low"],
        "high_risk_files":          [r.rel_path for r in risk.high_risk_files()[:5]],
        "circular_dependency_count":len(dep.circular_deps),
        "syntax_errors":            len(errors.by_type("syntax_error")),
        "entry_points":             dep.entry_points[:6],
        "dead_files":               dep.dead_files[:6],
        "most_central_file":        dep.most_central,
        "duplicate_pairs":          0,
        "analysis_time":            elapsed,
    }

    # Build files list
    files_out = []
    for rec in scan.files:
        risk_rec = risk.get(rec.rel_path)
        files_out.append({
            "path":     str(rec.path),
            "rel_path": rec.rel_path,
            "name":     rec.name,
            "ext":      rec.ext,
            "size":     rec.size,
            "language": rec.language,
            "category": rec.category,
            "complexity": {
                "level": rec.complexity_level or "low",
                "score": rec.complexity_score,
                # Fix: lines is always set by scanner; ensure non-zero
                "lines": max(rec.lines, 0),
            },
            "risk":     rec.risk or "low",
            "risk_reasoning": risk_rec.reasoning if risk_rec else [],
        })

    # ── Folder tree — hierarchical structure for graph ────────────────────
    # Build set of unique directory paths from all files
    folder_set: set[str] = set()
    for rec in scan.files:
        parts = rec.rel_path.replace('\\', '/').split('/')
        for i in range(1, len(parts)):
            folder_set.add('/'.join(parts[:i]))

    folder_tree = sorted(folder_set)   # deterministic order

    # Issues
    issues_out = [
        {
            "severity":   i.severity,
            "issue_type": i.issue_type,
            "file":       i.file,
            "message":    i.message,
            "line":       i.line,
        }
        for i in errors.issues
    ]

    # Dependencies
    deps_out = [{"source": e["source"], "target": e["target"]} for e in dep.edges]

    # Complexity
    complexity_out = [
        {
            "file":  rec.rel_path,
            "score": rec.complexity_score,
            "level": rec.complexity_level or "low",
            "lines": max(rec.lines, 0),
        }
        for rec in scan.files
    ]

    # Execution errors
    errors_out = []
    for r in trace.file_results.values():
        errors_out.append({
            "file":          r.rel_path,
            "compile_ok":    r.compile_ok,
            "compile_error": r.compile_error,
            "runtime_ok":    r.runtime_ok,
            "runtime_error": r.runtime_error,
            "timed_out":     r.timed_out,
            "is_entry_point":r.is_entry_point,
            "failure_chain": r.failure_chain,
        })

    return {
        "meta": {
            "root":              str(resolved),
            "analysis_time_sec": elapsed,
        },
        "summary":      summary,
        "files":        files_out,
        "folder_tree":  folder_tree,      # ← NEW: folder hierarchy for graph
        "dependencies": deps_out,
        "issues":       issues_out,
        "complexity":   complexity_out,
        "errors":       errors_out,
        "duplicates":   [],
    }


# ── Endpoints ─────────────────────────────────────────────────────────────────

@app.get("/health")
async def health():
    # API key presence check — never expose the key itself
    has_api_key = bool(os.environ.get("NVIDIA_API_KEY", "").strip())
    return {"status": "ok", "has_ai": HAS_AI, "has_graph": HAS_GRAPH, "has_api_key": has_api_key}


@app.post("/analyze")
async def analyze(req: AnalyzeRequest):
    """Run the full analysis pipeline and return JSON results."""
    if not os.path.isdir(req.path):
        raise HTTPException(
            status_code=400,
            detail=f"Path does not exist or is not a folder: {req.path}"
        )
    try:
        loop   = asyncio.get_event_loop()
        result = await loop.run_in_executor(
            None, lambda: _run_pipeline(req.path, req.safe_exec)
        )
        return JSONResponse(content=result)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        log.exception("Analysis failed: %s", e)
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/chat")
async def chat(req: ChatRequest):
    """
    Streaming AI chat endpoint (Server-Sent Events).
    API key is ONLY read from server environment — never from client.
    Streams:  data: {"content": "token"}\n\n
    Ends:     data: {"confidence": 85}\n\n
              data: [DONE]\n\n
    """
    if not HAS_AI:
        raise HTTPException(
            status_code=501,
            detail="AI modules not installed. Run: pip install openai"
        )

    # SECURITY: API key from server environment ONLY — never from client request
    api_key = os.environ.get("NVIDIA_API_KEY", "").strip()
    if not api_key:
        raise HTTPException(
            status_code=400,
            detail="NVIDIA_API_KEY not set on server. Add it to your .env file or environment."
        )

    async def event_stream():
        try:
            client = NIMClient(api_key)

            # Build system prompt
            system = (
                "You are a senior software engineer and code analysis assistant "
                "embedded in a local developer tool called CodeIntel. "
                "Answer questions about the user's project concisely and accurately. "
                "Use bullet points for lists. "
                "End every response with exactly: Confidence: <0-100>%"
            )
            if req.context_file:
                system += f"\nThe user is currently focused on file: {req.context_file}"

            # Add analysis context summary if available
            if _last_results:
                scan  = _last_results.get("scan")
                dep   = _last_results.get("dep")
                errors= _last_results.get("errors")
                risk  = _last_results.get("risk")
                if scan:
                    system += (
                        f"\n\nProject summary: {len(scan.files)} files, "
                        f"languages: {list(scan.language_counts.keys())}, "
                        f"total issues: {len(errors.issues) if errors else 0}, "
                        f"circular deps: {len(dep.circular_deps) if dep else 0}, "
                        f"entry points: {dep.entry_points[:4] if dep else []}, "
                        f"most central: {dep.most_central if dep else 'N/A'}."
                    )
                    if risk:
                        high = risk.high_risk_files()
                        if high:
                            system += f" High risk files: {[r.rel_path for r in high[:4]]}."

                # If user asked about a specific file, add its details
                if req.context_file and scan:
                    rec = scan.get_by_rel_path(req.context_file)
                    if rec and errors:
                        file_issues = errors.by_file(req.context_file)
                        if file_issues:
                            system += (
                                f"\nIssues in {req.context_file}: "
                                + "; ".join(i.message for i in file_issues[:4])
                            )

            # Build messages
            messages = []
            for h in req.history[-6:]:
                messages.append({"role": h.get("role", "user"), "content": h.get("content", "")})
            messages.append({"role": "user", "content": req.question})

            # Stream response
            full_text = []
            async for chunk in client.stream(messages, system_prompt=system):
                full_text.append(chunk)
                payload = json.dumps({"content": chunk})
                yield f"data: {payload}\n\n"
                await asyncio.sleep(0)

            # Parse confidence from full response
            conf = PromptEngine.parse_confidence("".join(full_text))
            yield f"data: {json.dumps({'confidence': conf})}\n\n"
            yield "data: [DONE]\n\n"

        except Exception as e:
            log.error("Chat error: %s", e)
            error_payload = json.dumps({"content": f"AI Error: {str(e)}"})
            yield f"data: {error_payload}\n\n"
            yield "data: [DONE]\n\n"

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control":    "no-cache",
            "X-Accel-Buffering":"no",
            "Connection":       "keep-alive",
        },
    )


@app.post("/impact")
async def impact(path: str, file: str):
    """Get impact analysis for a specific file."""
    try:
        if not _last_results:
            raise HTTPException(400, "No analysis results available. Run /analyze first.")
        dep = _last_results["dep"]
        return {
            "file":        file,
            "dependents":  dep.get_dependents(file),
            "dependencies":dep.get_dependencies(file),
            "in_cycle":    dep.is_in_cycle(file),
        }
    except Exception as e:
        raise HTTPException(500, str(e))


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000, reload=False)
