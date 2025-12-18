"""Cline (VSCode extension) session log parser."""

from __future__ import annotations

import json
from pathlib import Path

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, ToolCall, Turn


class ClineParser(BaseParser):
    """Parse Cline (VSCode extension) JSONL logs."""

    format = LogFormat.CLINE

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        entries = self._read_jsonl()
        session.turns = self._extract_turns(entries)
        session.metadata = {"format": "cline"}

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        if '"role": "user"' in sample or '"role": "assistant"' in sample:
            if '"tool_calls"' in sample or '"function_call"' in sample:
                return True
            if '"messages"' in sample:
                return True
        return False

    def _extract_turns(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []

        for entry in entries:
            if "messages" in entry:
                for msg in entry.get("messages", []):
                    turn = self._parse_message(msg)
                    if turn:
                        turns.append(turn)
            elif "role" in entry:
                turn = self._parse_message(entry)
                if turn:
                    turns.append(turn)

        return turns

    def _parse_message(self, msg: dict) -> Turn | None:
        role = msg.get("role")
        if role not in ("user", "assistant"):
            return None

        content = msg.get("content", "")
        if isinstance(content, list):
            parts = []
            for block in content:
                if isinstance(block, dict):
                    if block.get("type") == "text":
                        parts.append(block.get("text", ""))
                elif isinstance(block, str):
                    parts.append(block)
            content = "\n".join(parts)

        tool_calls = []
        if "tool_calls" in msg:
            for tc in msg.get("tool_calls", []):
                func = tc.get("function", {})
                tool_calls.append(
                    ToolCall(
                        name=func.get("name", "unknown"),
                        input=json.loads(func.get("arguments", "{}")),
                        id=tc.get("id", ""),
                    )
                )

        return Turn(
            role=role,
            content=content,
            tool_calls=tool_calls,
        )
