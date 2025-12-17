"""Built-in library plugins (DreamCoder-style abstraction management).

Libraries:
- MemoryLibrary: In-memory abstraction storage
- LearnedLibrary: Library that learns abstractions from solutions (future)
"""

from .memory import MemoryLibrary

__all__ = [
    "MemoryLibrary",
]
