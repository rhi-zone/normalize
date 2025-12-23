"""Built-in library plugins (DreamCoder-style abstraction management).

Libraries:
- MemoryLibrary: In-memory abstraction storage
- LearnedLibrary: Frequency-based abstraction learning

Base utilities:
- BaseLibrary: Shared functionality for library plugins
- extract_keywords: Keyword extraction for matching
- types_compatible: Type signature compatibility check
"""

from .base import BaseLibrary, extract_keywords, types_compatible
from .learned import (
    CodePattern,
    LearnedLibrary,
    PatternExtractor,
    PatternMatch,
    SolutionRecord,
)
from .memory import MemoryLibrary

__all__ = [
    "BaseLibrary",
    "CodePattern",
    "LearnedLibrary",
    "MemoryLibrary",
    "PatternExtractor",
    "PatternMatch",
    "SolutionRecord",
    "extract_keywords",
    "types_compatible",
]
