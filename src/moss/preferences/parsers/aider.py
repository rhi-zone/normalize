"""Aider chat log parser."""

from __future__ import annotations

import re
from pathlib import Path

from moss.preferences.parsers.base import BaseParser
from moss.preferences.parsing import LogFormat, ParsedSession, Turn


class AiderParser(BaseParser):
    """Parse Aider chat logs (markdown format)."""

    format = LogFormat.AIDER

    def parse(self) -> ParsedSession:
        session = ParsedSession(path=self.path, format=self.format)

        if not self.path.exists():
            return session

        content = self._read_file()
        session.turns = self._extract_turns(content)
        session.metadata = {"format": "aider"}

        return session

    @classmethod
    def can_parse(cls, path: Path, sample: str) -> bool:
        if "#### " in sample or ">" in sample:
            if re.search(r"^####\s+", sample, re.MULTILINE):
                return True
            if re.search(r"^>\s+", sample, re.MULTILINE):
                return True
        return False

    def _extract_turns(self, content: str) -> list[Turn]:
        turns: list[Turn] = []

        # Aider format: "#### user message" and "> assistant response"
        user_pattern = re.compile(r"^####\s*(.+?)(?=^####|\Z)", re.MULTILINE | re.DOTALL)

        user_matches = user_pattern.findall(content)
        for match in user_matches:
            turns.append(Turn(role="user", content=match.strip()))

        # If no structured format found, try line-by-line
        if not turns:
            lines = content.split("\n")
            current_role = None
            current_content: list[str] = []

            for line in lines:
                if line.startswith("User:") or line.startswith("Human:"):
                    if current_role and current_content:
                        turns.append(Turn(role=current_role, content="\n".join(current_content)))
                    current_role = "user"
                    current_content = [line.split(":", 1)[1].strip()]
                elif line.startswith("Assistant:") or line.startswith("AI:"):
                    if current_role and current_content:
                        turns.append(Turn(role=current_role, content="\n".join(current_content)))
                    current_role = "assistant"
                    current_content = [line.split(":", 1)[1].strip()]
                elif current_role:
                    current_content.append(line)

            if current_role and current_content:
                turns.append(Turn(role=current_role, content="\n".join(current_content)))

        return turns
