"""In-memory library plugin for abstraction storage.

Stores abstractions in memory with simple keyword-based search.
This is a basic implementation suitable for single sessions.

For persistent storage, see FileLibrary (future).
For learning capabilities, see LearnedLibrary (future).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from ..protocols import (
    Abstraction,
    LibraryMetadata,
    LibraryPlugin,
)
from .base import (
    BaseLibrary,
    extract_keywords,
    types_compatible,
)

if TYPE_CHECKING:
    from moss_orchestration.synthesis.types import Context, Specification


class MemoryLibrary(BaseLibrary):
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
        spec_keywords = extract_keywords(spec.description)
        if spec.type_signature:
            spec_keywords.update(extract_keywords(spec.type_signature))

        results: list[tuple[Abstraction, float]] = []

        for abstraction in self._abstractions.values():
            score = 0.0

            # Keyword overlap
            abs_keywords = extract_keywords(abstraction.description)
            abs_keywords.update(extract_keywords(abstraction.name))

            if spec_keywords and abs_keywords:
                overlap = len(spec_keywords & abs_keywords)
                score += overlap / max(len(spec_keywords), len(abs_keywords))

            # Type signature match
            if spec.type_signature and abstraction.type_signature:
                if spec.type_signature == abstraction.type_signature:
                    score += 0.5
                elif types_compatible(spec.type_signature, abstraction.type_signature):
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


# Protocol compliance check
assert isinstance(MemoryLibrary(), LibraryPlugin)
