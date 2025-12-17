"""In-memory library plugin for abstraction storage.

Stores abstractions in memory with simple keyword-based search.
This is a basic implementation suitable for single sessions.

For persistent storage, see FileLibrary (future).
For learning capabilities, see LearnedLibrary (future).
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING

from moss.synthesis.plugins.protocols import (
    Abstraction,
    LibraryMetadata,
    LibraryPlugin,
)

if TYPE_CHECKING:
    from moss.synthesis.types import Context, Specification


class MemoryLibrary:
    """In-memory abstraction library.

    Simple implementation that stores abstractions in memory and
    provides keyword-based search. Useful for:
    - Development and testing
    - Single-session synthesis
    - As a base for more sophisticated libraries

    Abstractions are matched by keyword overlap with specification
    description and type signature.
    """

    def __init__(self) -> None:
        self._abstractions: dict[str, Abstraction] = {}
        self._metadata = LibraryMetadata(
            name="memory",
            priority=0,
            description="In-memory abstraction storage",
            supports_learning=False,
            persistence_type="memory",
        )

    @property
    def metadata(self) -> LibraryMetadata:
        """Return library metadata."""
        return self._metadata

    def get_abstractions(self) -> list[Abstraction]:
        """Get all abstractions in the library."""
        return list(self._abstractions.values())

    def add_abstraction(self, abstraction: Abstraction) -> None:
        """Add an abstraction to the library.

        Args:
            abstraction: The abstraction to add
        """
        self._abstractions[abstraction.name] = abstraction

    def remove_abstraction(self, name: str) -> bool:
        """Remove an abstraction by name.

        Args:
            name: Abstraction name

        Returns:
            True if removed, False if not found
        """
        return self._abstractions.pop(name, None) is not None

    def _extract_keywords(self, text: str) -> set[str]:
        """Extract keywords from text for matching."""
        # Simple word extraction
        words = re.findall(r"\w+", text.lower())
        # Filter common words
        stopwords = {"a", "an", "the", "is", "are", "to", "for", "of", "in", "on", "and", "or"}
        return {w for w in words if len(w) > 2 and w not in stopwords}

    def search_abstractions(
        self,
        spec: Specification,
        context: Context,
    ) -> list[tuple[Abstraction, float]]:
        """Search for relevant abstractions.

        Scoring based on:
        - Keyword overlap with description
        - Type signature match
        - Usage count (frequently used = higher score)

        Args:
            spec: The specification to match
            context: Available resources

        Returns:
            List of (abstraction, score) pairs, sorted by relevance
        """
        if not self._abstractions:
            return []

        # Extract keywords from spec
        spec_keywords = self._extract_keywords(spec.description)
        if spec.type_signature:
            spec_keywords.update(self._extract_keywords(spec.type_signature))

        results: list[tuple[Abstraction, float]] = []

        for abstraction in self._abstractions.values():
            score = 0.0

            # Keyword overlap
            abs_keywords = self._extract_keywords(abstraction.description)
            abs_keywords.update(self._extract_keywords(abstraction.name))

            if spec_keywords and abs_keywords:
                overlap = len(spec_keywords & abs_keywords)
                score += overlap / max(len(spec_keywords), len(abs_keywords))

            # Type signature match
            if spec.type_signature and abstraction.type_signature:
                if spec.type_signature == abstraction.type_signature:
                    score += 0.5
                elif self._types_compatible(spec.type_signature, abstraction.type_signature):
                    score += 0.25

            # Boost for frequently used abstractions
            if abstraction.usage_count > 0:
                score += min(0.1 * abstraction.usage_count, 0.3)

            # Boost for high compression gain (DreamCoder metric)
            if abstraction.compression_gain > 0:
                score += min(abstraction.compression_gain * 0.1, 0.2)

            if score > 0:
                results.append((abstraction, score))

        # Sort by score descending
        results.sort(key=lambda x: x[1], reverse=True)

        return results

    def _types_compatible(self, type1: str, type2: str) -> bool:
        """Check if two type signatures are compatible.

        Simple heuristic: check if return types match.
        """

        # Extract return type (after ->)
        def get_return_type(sig: str) -> str | None:
            match = re.search(r"->\s*(\S+)", sig)
            return match.group(1) if match else None

        ret1 = get_return_type(type1)
        ret2 = get_return_type(type2)

        if ret1 and ret2:
            return ret1.lower() == ret2.lower()

        return False

    async def learn_abstraction(
        self,
        solutions: list[str],
        spec: Specification,
    ) -> Abstraction | None:
        """Memory library does not learn abstractions.

        For learning capabilities, use LearnedLibrary (future).
        """
        return None

    def record_usage(self, abstraction: Abstraction) -> None:
        """Record that an abstraction was used.

        Updates the usage count for the abstraction.
        """
        if abstraction.name in self._abstractions:
            old = self._abstractions[abstraction.name]
            # Create new abstraction with incremented count
            self._abstractions[abstraction.name] = Abstraction(
                name=old.name,
                code=old.code,
                type_signature=old.type_signature,
                description=old.description,
                usage_count=old.usage_count + 1,
                compression_gain=old.compression_gain,
            )

    def clear(self) -> None:
        """Clear all abstractions."""
        self._abstractions.clear()

    def __len__(self) -> int:
        """Return number of abstractions."""
        return len(self._abstractions)


# Protocol compliance check
assert isinstance(MemoryLibrary(), LibraryPlugin)
