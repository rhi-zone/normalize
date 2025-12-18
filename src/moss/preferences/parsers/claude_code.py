"""Claude Code session log parser."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, ToolCall, ToolResult, Turn


class ClaudeCodeParser(BaseParser):
    """Parse Claude Code JSONL session logs."""

    format = LogFormat.CLAUDE_CODE

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        entries = self._read_jsonl()
        session.turns = self._extract_turns(entries)
        session.metadata = self._extract_metadata(entries)

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        """Check for Claude Code format markers."""
        if '"type": "assistant"' in sample and '"requestId"' in sample:
            return True
        if '"type": "user"' in sample and '"message"' in sample:
            return True
        return False

    def _extract_turns(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []
        seen_request_ids: set[str] = set()

        for entry in entries:
            entry_type = entry.get("type")

            if entry_type == "user":
                message = entry.get("message", {})
                content = self._extract_text_content(message.get("content", []))
                turn = Turn(
                    role="user",
                    content=content,
                    timestamp=entry.get("timestamp"),
                )
                turns.append(turn)

            elif entry_type == "assistant":
                request_id = entry.get("requestId")
                if request_id and request_id in seen_request_ids:
                    for turn in reversed(turns):
                        if turn.request_id == request_id:
                            self._update_turn_from_entry(turn, entry)
                            break
                else:
                    if request_id:
                        seen_request_ids.add(request_id)
                    turn = self._create_assistant_turn(entry)
                    turns.append(turn)

        return turns

    def _create_assistant_turn(self, entry: dict) -> Turn:
        message = entry.get("message", {})
        content_blocks = message.get("content", [])

        text_content = self._extract_text_content(content_blocks)
        tool_calls = self._extract_tool_calls(content_blocks)
        tool_results = self._extract_tool_results(content_blocks)

        return Turn(
            role="assistant",
            content=text_content,
            tool_calls=tool_calls,
            tool_results=tool_results,
            timestamp=entry.get("timestamp"),
            request_id=entry.get("requestId"),
        )

    def _update_turn_from_entry(self, turn: Turn, entry: dict) -> None:
        message = entry.get("message", {})
        content_blocks = message.get("content", [])

        text = self._extract_text_content(content_blocks)
        if text:
            turn.content = text

        new_calls = self._extract_tool_calls(content_blocks)
        existing_ids = {tc.id for tc in turn.tool_calls}
        for tc in new_calls:
            if tc.id not in existing_ids:
                turn.tool_calls.append(tc)

        new_results = self._extract_tool_results(content_blocks)
        existing_result_ids = {tr.tool_use_id for tr in turn.tool_results}
        for tr in new_results:
            if tr.tool_use_id not in existing_result_ids:
                turn.tool_results.append(tr)

    def _extract_text_content(self, content_blocks: list | str) -> str:
        if isinstance(content_blocks, str):
            return content_blocks

        parts = []
        for block in content_blocks:
            if isinstance(block, str):
                parts.append(block)
            elif isinstance(block, dict):
                if block.get("type") == "text":
                    parts.append(block.get("text", ""))
        return "\n".join(parts)

    def _extract_tool_calls(self, content_blocks: list) -> list[ToolCall]:
        calls = []
        if not isinstance(content_blocks, list):
            return calls

        for block in content_blocks:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                calls.append(
                    ToolCall(
                        name=block.get("name", "unknown"),
                        input=block.get("input", {}),
                        id=block.get("id", ""),
                    )
                )
        return calls

    def _extract_tool_results(self, content_blocks: list) -> list[ToolResult]:
        results = []
        if not isinstance(content_blocks, list):
            return results

        for block in content_blocks:
            if isinstance(block, dict) and block.get("type") == "tool_result":
                content = block.get("content", "")
                if isinstance(content, list):
                    content = str(content)
                results.append(
                    ToolResult(
                        tool_use_id=block.get("tool_use_id", ""),
                        content=str(content),
                        is_error=block.get("is_error", False),
                    )
                )
        return results

    def _extract_metadata(self, entries: list[dict]) -> dict[str, Any]:
        metadata: dict[str, Any] = {"format": "claude_code"}

        for entry in entries:
            if entry.get("type") == "system":
                if "cwd" in entry:
                    metadata["cwd"] = entry["cwd"]
                if "model" in entry:
                    metadata["model"] = entry["model"]

        user_count = sum(1 for e in entries if e.get("type") == "user")
        assistant_count = len({e.get("requestId") for e in entries if e.get("type") == "assistant"})

        metadata["user_messages"] = user_count
        metadata["assistant_messages"] = assistant_count

        return metadata
