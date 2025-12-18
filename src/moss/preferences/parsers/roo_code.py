"""Roo Code session log parser."""

from __future__ import annotations

from pathlib import Path

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, Turn


class RooCodeParser(BaseParser):
    """Parse Roo Code JSONL logs."""

    format = LogFormat.ROO_CODE

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        entries = self._read_jsonl()
        session.turns = self._extract_turns(entries)
        session.metadata = {"format": "roo_code"}

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        if "roo" in sample.lower() or "roocode" in sample.lower():
            return True
        return False

    def _extract_turns(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []

        for entry in entries:
            role = entry.get("role") or entry.get("type")
            if role == "human" or role == "user":
                role = "user"
            elif role == "ai" or role == "assistant":
                role = "assistant"
            else:
                continue

            content = entry.get("content", "") or entry.get("text", "")
            if isinstance(content, list):
                content = "\n".join(str(c) for c in content)

            turns.append(Turn(role=role, content=str(content)))

        return turns
