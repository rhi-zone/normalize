"""Generic session log parsers (fallback)."""

from __future__ import annotations

import re
from pathlib import Path

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, Turn


class GenericJSONLParser(BaseParser):
    """Parse generic JSONL chat logs."""

    format = LogFormat.GENERIC_JSONL

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        entries = self._read_jsonl()
        session.turns = self._extract_turns(entries)
        session.metadata = {"format": "generic_jsonl"}

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        # Check if it looks like JSONL with role/content
        if '{"' in sample or "{\n" in sample:
            if '"role"' in sample or '"content"' in sample:
                return True
        return False

    def _extract_turns(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []

        for entry in entries:
            role = (
                entry.get("role") or entry.get("type") or entry.get("sender") or entry.get("from")
            )

            if role in ("user", "human", "User", "Human"):
                role = "user"
            elif role in ("assistant", "ai", "bot", "Assistant", "AI", "model"):
                role = "assistant"
            else:
                continue

            content = entry.get("content") or entry.get("text") or entry.get("message") or ""

            if isinstance(content, list):
                parts = []
                for item in content:
                    if isinstance(item, dict):
                        parts.append(item.get("text", str(item)))
                    else:
                        parts.append(str(item))
                content = "\n".join(parts)

            turns.append(Turn(role=role, content=str(content)))

        return turns


class GenericChatParser(BaseParser):
    """Parse generic chat logs with common patterns (fallback parser)."""

    format = LogFormat.GENERIC_CHAT

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        # Try JSONL first, then text
        entries = self._read_jsonl()
        if entries:
            session.turns = self._extract_from_jsonl(entries)
        else:
            content = self._read_file()
            session.turns = self._extract_from_text(content)

        session.metadata = {"format": "generic"}
        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        # Generic parser is always the fallback
        return True

    def _extract_from_jsonl(self, entries: list[dict]) -> list[Turn]:
        turns: list[Turn] = []

        for entry in entries:
            role = (
                entry.get("role") or entry.get("type") or entry.get("sender") or entry.get("from")
            )

            if role in ("user", "human", "User", "Human"):
                role = "user"
            elif role in ("assistant", "ai", "bot", "Assistant", "AI", "model"):
                role = "assistant"
            else:
                continue

            content = entry.get("content") or entry.get("text") or entry.get("message") or ""

            if isinstance(content, list):
                parts = []
                for item in content:
                    if isinstance(item, dict):
                        parts.append(item.get("text", str(item)))
                    else:
                        parts.append(str(item))
                content = "\n".join(parts)

            turns.append(Turn(role=role, content=str(content)))

        return turns

    def _extract_from_text(self, content: str) -> list[Turn]:
        turns: list[Turn] = []

        patterns = [
            (r"^User:\s*(.+?)(?=^(?:User|Assistant|AI|Human):|$)", "user"),
            (r"^Human:\s*(.+?)(?=^(?:User|Assistant|AI|Human):|$)", "user"),
            (r"^Assistant:\s*(.+?)(?=^(?:User|Assistant|AI|Human):|$)", "assistant"),
            (r"^AI:\s*(.+?)(?=^(?:User|Assistant|AI|Human):|$)", "assistant"),
        ]

        for pattern, role in patterns:
            matches = re.findall(pattern, content, re.MULTILINE | re.DOTALL | re.IGNORECASE)
            for match in matches:
                turns.append(Turn(role=role, content=match.strip()))

        return turns
