"""Gemini CLI session log parser."""

from __future__ import annotations

from pathlib import Path

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, ToolCall, Turn


class GeminiCLIParser(BaseParser):
    """Parse Gemini CLI logs."""

    format = LogFormat.GEMINI_CLI

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        entries = self._read_jsonl()
        session.turns = self._extract_turns(entries)
        session.metadata = {"format": "gemini_cli"}

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        if "gemini" in sample.lower():
            return True
        if '"model": "gemini' in sample.lower():
            return True
        return False

    def _extract_turns(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []

        for entry in entries:
            role = entry.get("role")
            if role == "user":
                content = entry.get("parts", [{}])[0].get("text", "")
                turns.append(Turn(role="user", content=content))
            elif role == "model":
                content = entry.get("parts", [{}])[0].get("text", "")
                tool_calls = []
                for part in entry.get("parts", []):
                    if "functionCall" in part:
                        fc = part["functionCall"]
                        tool_calls.append(
                            ToolCall(
                                name=fc.get("name", "unknown"),
                                input=fc.get("args", {}),
                                id=str(hash(str(fc))),
                            )
                        )
                turns.append(Turn(role="assistant", content=content, tool_calls=tool_calls))

        return turns
