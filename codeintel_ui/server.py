"""
server.py  —  FastAPI bridge for the CodeIntel React UI
Run: uvicorn server:app --reload --port 8000
"""
import os, sys, json, tempfile, asyncio
from pathlib import Path
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse, JSONResponse
from pydantic import BaseModel

sys.path.insert(0, str(Path(__file__).parent))
from analyzer import analyze_project

app = FastAPI(title="CodeIntel API", version="2.0.0")
app.add_middleware(
    CORSMiddleware, allow_origins=["*"],
    allow_methods=["*"], allow_headers=["*"],
)


# ── Schemas ───────────────────────────────────────────────────────────────────

class AnalyzeRequest(BaseModel):
    path:              str
    safe_exec:         bool = False
    detect_duplicates: bool = False

class ChatRequest(BaseModel):
    question:     str
    context_file: str | None = None
    api_key:      str | None = None
    history:      list[dict] = []


# ── Endpoints ─────────────────────────────────────────────────────────────────

@app.get("/health")
async def health():
    return {"status": "ok"}


@app.post("/analyze")
async def analyze(req: AnalyzeRequest):
    if not os.path.isdir(req.path):
        raise HTTPException(400, f"Not a directory: {req.path}")
    try:
        graph_tmp = tempfile.mktemp(suffix=".png")
        result    = analyze_project(
            root_path=req.path,
            safe_exec=req.safe_exec,
            detect_duplicates=req.detect_duplicates,
            graph_image=graph_tmp,
        )
        # Embed graph PNG as base64 if it was generated
        if Path(graph_tmp).exists():
            import base64
            with open(graph_tmp, "rb") as f:
                result["graph_image_b64"] = base64.b64encode(f.read()).decode()
            os.unlink(graph_tmp)
        return JSONResponse(content=result)
    except Exception as e:
        raise HTTPException(500, str(e))


@app.post("/chat")
async def chat(req: ChatRequest):
    """
    Streaming chat endpoint.
    Streams SSE: data: {"content": "..."}\n\n
    Ends with:   data: {"confidence": 85}\n\n
                 data: [DONE]\n\n
    """
    api_key = req.api_key or os.environ.get("NVIDIA_API_KEY", "")
    if not api_key:
        raise HTTPException(400, "No NVIDIA API key provided")

    async def event_stream():
        try:
            from ai.nim_client   import NIMClient
            from ai.prompt_engine import PromptEngine

            # Build a minimal context string if no full analysis data
            prompt_engine = PromptEngine()
            client        = NIMClient(api_key)

            # Simple prompt — full ContextBuilder needs analysis results
            # which are not persisted server-side between requests here.
            # The React hook sends history + question; we forward that directly.
            messages = list(req.history) + [{"role": "user", "content": req.question}]

            system = (
                "You are a senior software engineer and code analysis assistant. "
                "Answer questions about the user's project concisely and accurately. "
                "End every response with: Confidence: <0-100>%"
            )
            if req.context_file:
                system += f"\nThe user is currently focused on file: {req.context_file}"

            full_text = []
            async for chunk in client.stream(messages, system_prompt=system):
                full_text.append(chunk)
                payload = json.dumps({"content": chunk})
                yield f"data: {payload}\n\n"
                await asyncio.sleep(0)

            # Parse confidence
            from ai.prompt_engine import PromptEngine as PE
            conf = PE.parse_confidence("".join(full_text))
            yield f"data: {json.dumps({'confidence': conf})}\n\n"
            yield "data: [DONE]\n\n"

        except Exception as e:
            yield f"data: {json.dumps({'content': f'Error: {e}'})}\n\n"
            yield "data: [DONE]\n\n"

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
