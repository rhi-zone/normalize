"""Base classes and utilities for library plugins.

Shared functionality used by MemoryLibrary and LearnedLibrary.
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING

from ..protocols import Abstraction, LibraryMetadata

if TYPE_CHECKING:
    pass


# Common stopwords for keyword extraction
STOPWORDS = frozenset({"a", "an", "the", "is", "are", "to", "for", "of", "in", "on", "and", "or"})


def extract_keywords(text: str) -> set[str]:
    """Extract keywords from text for matching.

    Extracts words >2 chars, lowercased, excluding stopwords.
    """
    words = re.findall(r"\w+", text.lower())
    return {w for w in words if len(w) > 2 and w not in STOPWORDS}


def get_return_type(sig: str) -> str | None:
    """Extract return type from a type signature.

    Args:
        sig: Type signature like "(int, int) -> int"

    Returns:
        Return type string or None if not found
    """
    match = re.search(r"->\s*(\S+)", sig)
    return match.group(1) if match else None


def types_compatible(type1: str, type2: str) -> bool:
    """Check if two type signatures are compatible.

    Simple heuristic: check if return types match (case-insensitive).
    """
    ret1 = get_return_type(type1)
    ret2 = get_return_type(type2)

    if ret1 and ret2:
        return ret1.lower() == ret2.lower()
    return False


class BaseLibrary:
    """Base class for library plugins with common functionality.

    Provides shared implementations for:
    - metadata property
    - __len__ for abstraction count
    - keyword extraction and type compatibility

    Subclasses must set self._abstractions and self._metadata in __init__.
    """

    _abstractions: dict[str, Abstraction]
    _metadata: LibraryMetadata

    @property
    def metadata(self) -> LibraryMetadata:
        """Return library metadata."""
        return self._metadata

    def __len__(self) -> int:
        """Return number of abstractions."""
        return len(self._abstractions)
