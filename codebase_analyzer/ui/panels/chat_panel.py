"""
ui/panels/chat_panel.py
Phase 6: Live AI chat panel with streaming response display.

Renders conversation history, streams AI tokens in real-time,
shows confidence score, and posts MessageSent to main_window which
orchestrates the full AI pipeline.
"""

from datetime import datetime
from textual.app import ComposeResult
from textual.widget import Widget
from textual.widgets import Static, Input, Button, Label
from textual.containers import Vertical, Horizontal, ScrollableContainer
from textual.reactive import reactive
from rich.text import Text

from utils.logger import get_logger

log = get_logger(__name__)

# ─── Suggested starter questions ─────────────────────────────────────────────

SUGGESTED_QUESTIONS = [
    "What are the highest-risk files?",
    "Explain the circular dependency",
    "Which file would break the most if it failed?",
    "What errors were detected?",
    "Which files are entry points?",
]

# ─── Bubble renderers ─────────────────────────────────────────────────────────

def _ts() -> str:
    return datetime.now().strftime("%H:%M")


def _user_bubble(text: str) -> Text:
    t = Text()
    t.append(f"\n  ▶ You  [{_ts()}]\n", style="bold cyan")
    t.append(f"  {text}\n", style="white")
    return t


def _ai_bubble_header(model_short: str) -> Text:
    t = Text()
    t.append(f"\n  ★ AI", style="bold magenta")
    t.append(f"  [{_ts()}]", style="dim")
    if model_short:
        t.append(f"  {model_short}", style="dim cyan")
    t.append("\n")
    return t


def _ai_confidence_line(confidence: int | None) -> Text:
    t = Text()
    if confidence is None:
        return t
    if confidence >= 80:
        style = "bold green"
    elif confidence >= 50:
        style = "bold yellow"
    else:
        style = "bold red"
    t.append(f"  Confidence: {confidence}%\n", style=style)
    return t


def _system_bubble(text: str) -> Text:
    t = Text()
    t.append(f"  ─  {text}\n", style="dim")
    return t


def _error_bubble(text: str) -> Text:
    t = Text()
    t.append(f"  ✖ {text}\n", style="bold red")
    return t


# ─── Panel ────────────────────────────────────────────────────────────────────

class ChatPanel(Widget):
    """
    Phase 6 AI chat panel.

    Key behaviours:
    - MessageSent posted to main_window which runs the full AI pipeline
    - main_window calls stream_start(), stream_chunk(), stream_end()
      as AI tokens arrive — panel updates live
    - Confidence score displayed after each AI reply
    - Conversation history preserved for follow-up context
    - Focus file context shown in bar at top
    """

    is_thinking: reactive[bool] = reactive(False)

    DEFAULT_CSS = """
    ChatPanel {
        width: 36;
        min-width: 26;
        background: $panel;
        border-left: tall $primary-darken-3;
        layout: vertical;
    }

    #chat-title {
        background: $secondary-darken-2;
        color: $text;
        padding: 0 1;
        text-style: bold;
        height: 1;
        width: 100%;
    }

    #context-bar {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $surface;
    }

    #model-bar {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $panel-darken-1;
    }

    #chat-history {
        height: 1fr;
        overflow-y: auto;
        scrollbar-size: 1 1;
        background: $surface;
        padding: 0 1;
    }

    #chat-content { padding: 0; }

    #suggestions {
        height: 3;
        margin: 0 1;
        overflow-x: auto;
    }

    .suggestion-btn {
        background: $panel-darken-1;
        border: none;
        color: $text-muted;
        margin-right: 1;
        padding: 0 1;
        width: auto;
    }

    .suggestion-btn:hover {
        background: $boost;
        color: $text;
    }

    #input-row {
        height: 3;
        margin: 0 1 1 1;
    }

    #chat-input {
        width: 1fr;
        margin-right: 1;
    }

    #btn-send {
        width: 7;
        background: $secondary-darken-1;
        border: none;
        color: $text;
        text-style: bold;
    }

    #btn-send:hover       { background: $secondary; }
    #btn-send.--thinking  { background: $warning-darken-2; color: $text-muted; }

    #btn-clear-chat {
        width: 7;
        background: $panel-darken-1;
        border: none;
        color: $text-muted;
        margin-left: 1;
    }

    #btn-clear-chat:hover { background: $error-darken-2; color: $text; }

    #chat-status {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $panel-darken-1;
    }
    """

    # ── Messages posted to main_window ────────────────────────────────────────

    class MessageSent:
        """Posted when user submits a message. main_window runs the AI pipeline."""
        def __init__(self, text: str, context_file: str | None, history: list[dict]):
            self.text         = text
            self.context_file = context_file
            self.history      = history    # prior conversation turns

    class ClearRequested:
        pass

    # ── Init ──────────────────────────────────────────────────────────────────

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._history_lines: list[Text] = []
        self._context_file: str | None  = None
        self._ai_connected  = False
        self._model_name    = ""
        self._is_fallback   = False
        # Conversation turns for follow-up context
        self._turns: list[dict] = []
        # Current streaming state
        self._streaming_text = ""

    # ── Compose ───────────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        yield Static(" 🤖 AI ASSISTANT", id="chat-title")
        yield Static(" Context: none", id="context-bar")
        yield Static(" Model: not connected", id="model-bar")

        with ScrollableContainer(id="chat-history"):
            yield Static(id="chat-content")

        with Horizontal(id="suggestions"):
            for q in SUGGESTED_QUESTIONS[:3]:
                short = q[:20] + "…" if len(q) > 20 else q
                yield Button(short, classes="suggestion-btn")

        with Horizontal(id="input-row"):
            yield Input(placeholder="Ask about your project…", id="chat-input")
            yield Button("Send",  id="btn-send", variant="primary")
            yield Button("Clear", id="btn-clear-chat")

        yield Static("AI not connected — run Phase 6 setup", id="chat-status")

    def on_mount(self) -> None:
        self._show_welcome()

    # ── Public API (called by main_window) ────────────────────────────────────

    def set_ai_connected(self, model_name: str, is_fallback: bool = False) -> None:
        """Called after successful NIMClient.health_check()."""
        self._ai_connected = True
        self._model_name   = model_name
        self._is_fallback  = is_fallback

        short = model_name.split("/")[-1][:24]
        fallback_note = " (fallback)" if is_fallback else ""
        self.query_one("#model-bar", Static).update(
            f" Model: [bold cyan]{short}[/bold cyan]{fallback_note}"
        )
        self.query_one("#chat-status", Static).update(
            f" ✔ Connected to {short}"
        )
        self.append_system_message(f"Connected: {model_name}")

    def set_ai_disconnected(self, reason: str = "") -> None:
        self._ai_connected = False
        self.query_one("#model-bar", Static).update(" Model: disconnected")
        msg = f"AI disconnected{': ' + reason if reason else ''}"
        self.query_one("#chat-status", Static).update(f" ✖ {msg}")
        self.append_system_message(msg)

    def inject_file_context(self, rel_path: str) -> None:
        """Called when user clicks a file in the file explorer."""
        self._context_file = rel_path
        self.query_one("#context-bar", Static).update(
            f" Context: [bold cyan]{rel_path}[/bold cyan]"
        )
        self.append_system_message(f"Context set → {rel_path}")

    def append_user_message(self, text: str) -> None:
        self._history_lines.append(_user_bubble(text))
        self._refresh_history()

    def append_ai_message(self, text: str, confidence: int | None = None) -> None:
        """Append a complete (non-streaming) AI message."""
        self.set_thinking(False)
        header = _ai_bubble_header(self._model_name.split("/")[-1][:18])
        body   = Text(f"  {text.strip()}\n", style="bright_white")
        conf   = _ai_confidence_line(confidence)
        combined = Text()
        combined.append_text(header)
        combined.append_text(body)
        combined.append_text(conf)
        self._history_lines.append(combined)
        self._refresh_history()

    def append_system_message(self, text: str) -> None:
        self._history_lines.append(_system_bubble(text))
        self._refresh_history()

    def append_error_message(self, text: str) -> None:
        self.set_thinking(False)
        self._history_lines.append(_error_bubble(text))
        self._refresh_history()

    # ── Streaming API ─────────────────────────────────────────────────────────

    def stream_start(self) -> None:
        """Called when AI starts streaming. Adds a pending AI bubble."""
        self._streaming_text = ""
        header = _ai_bubble_header(self._model_name.split("/")[-1][:18])
        pending = Text()
        pending.append_text(header)
        pending.append("  [cyan]▌[/cyan]\n")   # blinking cursor placeholder
        self._history_lines.append(pending)
        self._refresh_history()

    def stream_chunk(self, chunk: str) -> None:
        """Called for each streamed token chunk. Updates the last bubble live."""
        self._streaming_text += chunk
        if self._history_lines:
            header = _ai_bubble_header(self._model_name.split("/")[-1][:18])
            updated = Text()
            updated.append_text(header)
            updated.append(f"  {self._streaming_text}[cyan]▌[/cyan]\n", style="bright_white")
            self._history_lines[-1] = updated
            self._refresh_history()

    def stream_end(self, confidence: int | None = None) -> None:
        """Called when streaming is complete. Finalises the bubble."""
        self.set_thinking(False)
        full_text = self._streaming_text

        # Strip confidence line from display text
        from ai.prompt_engine import PromptEngine
        clean_text = PromptEngine.strip_confidence(full_text)

        if self._history_lines:
            header = _ai_bubble_header(self._model_name.split("/")[-1][:18])
            final  = Text()
            final.append_text(header)
            final.append(f"  {clean_text.strip()}\n", style="bright_white")
            final.append_text(_ai_confidence_line(confidence))
            self._history_lines[-1] = final
            self._refresh_history()

        # Save to conversation turns for follow-up context
        self._turns.append({"role": "assistant", "content": full_text})
        self._streaming_text = ""

    # ── State ─────────────────────────────────────────────────────────────────

    def set_thinking(self, thinking: bool) -> None:
        self.is_thinking = thinking
        btn = self.query_one("#btn-send", Button)
        if thinking:
            btn.label    = "…"
            btn.disabled = True
            btn.add_class("--thinking")
            self.query_one("#chat-status", Static).update(" AI is thinking…")
        else:
            btn.label    = "Send"
            btn.disabled = False
            btn.remove_class("--thinking")
            status = (
                f" ✔ {self._model_name.split('/')[-1][:24]}"
                if self._ai_connected else
                " AI not connected"
            )
            self.query_one("#chat-status", Static).update(status)

    def clear_history(self) -> None:
        self._history_lines.clear()
        self._turns.clear()
        self._streaming_text = ""
        self._refresh_history()
        self.append_system_message("Conversation cleared")

    # ── Event handlers ────────────────────────────────────────────────────────

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()
        if event.button.id == "btn-send":
            self._handle_send()
        elif event.button.id == "btn-clear-chat":
            self.clear_history()
        elif "suggestion-btn" in event.button.classes:
            self.query_one("#chat-input", Input).value = str(event.button.label).rstrip("…")
            self._handle_send()

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "chat-input":
            self._handle_send()

    # ── Internal ─────────────────────────────────────────────────────────────

    def _handle_send(self) -> None:
        inp  = self.query_one("#chat-input", Input)
        text = inp.value.strip()
        if not text or self.is_thinking:
            return
        inp.value = ""

        self.append_user_message(text)
        self._turns.append({"role": "user", "content": text})

        if self._ai_connected:
            self.set_thinking(True)
            self.post_message(
                self.MessageSent(text, self._context_file, list(self._turns[:-1]))
            )
        else:
            self._history_lines.append(_error_bubble(
                "AI not connected. Set NVIDIA_API_KEY and restart."
            ))
            self._refresh_history()

    def _refresh_history(self) -> None:
        combined = Text()
        for line in self._history_lines:
            combined.append_text(line)
        self.query_one("#chat-content", Static).update(combined)
        try:
            self.query_one("#chat-history", ScrollableContainer).scroll_end(animate=False)
        except Exception:
            pass

    def _show_welcome(self) -> None:
        t = Text()
        t.append("\n  Code Intelligence Assistant\n", style="bold magenta")
        t.append("  ──────────────────────────\n", style="dim")
        t.append("  Analyze a project first,\n", style="dim")
        t.append("  then ask questions about\n", style="dim")
        t.append("  its structure and risks.\n\n", style="dim")
        t.append("  Requires: NVIDIA API key\n", style="dim cyan")
        t.append("  Model: Llama 3.1 70B\n", style="dim cyan")
        self._history_lines.append(t)
        self._refresh_history()
