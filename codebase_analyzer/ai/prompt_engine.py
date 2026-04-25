"""
ai/prompt_engine.py
Phase 6: Prompt formatter and response parser.

Takes a ProjectContext from ContextBuilder and produces:
  - A system prompt that defines the AI's role and constraints
  - A formatted user message with structured project data
  - Parses confidence score from AI response

Public API:
    engine  = PromptEngine()
    payload = engine.build_payload(context)    -> (system: str, user: str)
    conf    = engine.parse_confidence(reply)   -> int | None
    clean   = engine.strip_confidence(reply)   -> str
"""

import re
from dataclasses import dataclass

from utils.logger import get_logger
from ai.context_builder import ProjectContext, FileContext

log = get_logger(__name__)


# ─── System prompt ────────────────────────────────────────────────────────────

SYSTEM_PROMPT = """\
You are a senior software engineer and code analysis assistant integrated into
a local developer tool called "Codebase Analyzer".

Your role:
- Answer questions about the user's specific project using ONLY the context provided.
- Be precise, technical, and actionable.
- Never hallucinate file names, function names, or issues not present in the context.
- If the context does not contain enough information to answer confidently, say so clearly.

Response format:
- Answer the question directly in the first paragraph.
- Use bullet points for lists of files, issues, or recommendations.
- End your response with a single line in exactly this format:
  Confidence: <0–100>%
  (where 0 = no data to answer, 100 = full certainty from provided context)

Constraints:
- Maximum response length: ~400 words.
- Do NOT suggest external tools or services.
- Do NOT reference information outside the provided project context.
- Do NOT repeat the question back to the user.
"""


# ─── Template helpers ─────────────────────────────────────────────────────────

def _section(title: str, body: str) -> str:
    return f"\n## {title}\n{body.strip()}\n"


def _fmt_file(fc: FileContext) -> str:
    lines = [
        f"### {fc.rel_path}",
        f"Language: {fc.language}  |  Risk: {fc.risk_level.upper()}  |  Lines: {fc.lines}",
    ]

    if fc.is_entry:
        lines.append("Role: ENTRY POINT")

    if fc.imports:
        lines.append(f"Imports: {', '.join(fc.imports[:8])}")

    if fc.imported_by:
        lines.append(f"Imported by: {', '.join(fc.imported_by[:5])}")

    if fc.issues:
        lines.append("Issues:")
        for issue in fc.issues[:4]:
            lines.append(f"  - {issue}")

    if fc.reasoning:
        lines.append("Risk reasons:")
        for reason in fc.reasoning[:3]:
            lines.append(f"  - {reason}")

    if fc.snippet:
        # Only include snippet if it adds value (has real code)
        snippet_lines = fc.snippet.strip().splitlines()
        # Take first 20 lines max to keep context tight
        trimmed = "\n".join(snippet_lines[:20])
        if len(snippet_lines) > 20:
            trimmed += "\n… (truncated)"
        lines.append(f"Source snippet:\n```{fc.language.lower()}\n{trimmed}\n```")

    return "\n".join(lines)


# ─── Engine ───────────────────────────────────────────────────────────────────

@dataclass
class PromptPayload:
    system: str
    user:   str
    estimated_tokens: int


class PromptEngine:
    """
    Formats a ProjectContext into a (system, user) message pair
    ready to send to NIMClient.
    """

    def build_payload(self, context: ProjectContext) -> PromptPayload:
        """
        Build the full prompt payload from a ProjectContext.
        Returns system prompt + structured user message.
        """
        user_msg = self._build_user_message(context)
        log.debug(
            "Prompt built: ~%d chars, %d files in context",
            len(user_msg), len(context.files),
        )
        return PromptPayload(
            system=SYSTEM_PROMPT,
            user=user_msg,
            estimated_tokens=context.estimated_tokens,
        )

    def build_messages(self, context: ProjectContext) -> list[dict]:
        """
        Return messages list ready for NIMClient.stream() or NIMClient.complete().
        Includes the system prompt as the first message.
        """
        payload = self.build_payload(context)
        return [
            {"role": "system",    "content": payload.system},
            {"role": "user",      "content": payload.user},
        ]

    def build_followup_messages(
        self,
        context:  ProjectContext,
        history:  list[dict],
    ) -> list[dict]:
        """
        Build messages for a follow-up question that includes prior conversation.
        history: list of {"role": "user"/"assistant", "content": "..."}
        """
        payload = self.build_payload(context)
        messages = [{"role": "system", "content": payload.system}]

        # Include abbreviated prior turns (keep last 4 turns = 8 messages max)
        for msg in history[-8:]:
            messages.append(msg)

        # Add new question with fresh context
        messages.append({"role": "user", "content": payload.user})
        return messages

    # ── Response parsing ──────────────────────────────────────────────────────

    @staticmethod
    def parse_confidence(response: str) -> int | None:
        """
        Extract confidence score from AI response.
        Expected format: "Confidence: 85%"
        Returns None if not found.
        """
        match = re.search(r'[Cc]onfidence:\s*(\d{1,3})\s*%', response)
        if match:
            val = int(match.group(1))
            return max(0, min(100, val))   # clamp to 0–100
        return None

    @staticmethod
    def strip_confidence(response: str) -> str:
        """Remove the confidence line from the response for clean display."""
        return re.sub(r'\n?[Cc]onfidence:\s*\d{1,3}\s*%\s*$', '', response).rstrip()

    # ── Internal message building ─────────────────────────────────────────────

    @staticmethod
    def _build_user_message(ctx: ProjectContext) -> str:
        parts: list[str] = []

        # ── User question ─────────────────────────────────────────────────────
        parts.append(_section("User Question", ctx.question))

        # ── Project summary ───────────────────────────────────────────────────
        lang_str = ", ".join(
            f"{lang} {pct:.0f}%"
            for lang, pct in sorted(ctx.language_dist.items(), key=lambda x: -x[1])
        )
        summary_lines = [
            f"Root:          {ctx.project_root}",
            f"Total files:   {ctx.total_files}",
            f"Languages:     {lang_str}",
            f"Total issues:  {ctx.total_issues}",
            f"Circular deps: {ctx.circular_deps}",
            f"Entry points:  {', '.join(ctx.entry_points) or 'none detected'}",
            f"Most central:  {ctx.most_central or 'N/A'}",
        ]
        if ctx.high_risk_files:
            summary_lines.append(f"High-risk:     {', '.join(ctx.high_risk_files[:4])}")

        parts.append(_section("Project Summary", "\n".join(summary_lines)))

        # ── Focus file note ───────────────────────────────────────────────────
        if ctx.focus_file:
            parts.append(_section("User Focus File", ctx.focus_file))

        # ── Global issues ─────────────────────────────────────────────────────
        if ctx.global_issues:
            issues_str = "\n".join(f"- {i}" for i in ctx.global_issues)
            parts.append(_section("Global Issues", issues_str))

        # ── Failure chains ────────────────────────────────────────────────────
        if ctx.failure_chains:
            chain_lines = []
            for chain in ctx.failure_chains[:3]:
                root    = chain["root"]
                impacts = chain.get("impacts", [])
                err     = chain.get("error", "")
                chain_lines.append(
                    f"- Root failure: {root}"
                    + (f" ({err[:60]})" if err else "")
                    + (f"\n  Impacted: {', '.join(impacts[:5])}" if impacts else "")
                )
            parts.append(_section("Execution Failure Chains", "\n".join(chain_lines)))

        # ── Relevant files ────────────────────────────────────────────────────
        if ctx.files:
            file_blocks = [_fmt_file(fc) for fc in ctx.files]
            parts.append(_section(
                f"Relevant Files ({len(ctx.files)} selected)",
                "\n\n".join(file_blocks),
            ))
        else:
            parts.append(_section("Relevant Files", "No file context selected."))

        # ── Instructions reminder ─────────────────────────────────────────────
        parts.append(_section(
            "Instructions",
            "Answer the question above using ONLY the context provided.\n"
            "End with: Confidence: <0-100>%",
        ))

        return "".join(parts)
