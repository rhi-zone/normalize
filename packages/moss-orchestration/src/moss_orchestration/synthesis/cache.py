"""Caching utilities for synthesis.

Provides caching for expensive operations during synthesis:
- Test execution results
- Subproblem solutions
- Strategy selection results
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from typing import Any

from .types import Specification


@dataclass
class CacheEntry:
    """A cached result with metadata."""

    value: Any
    created_at: datetime = field(default_factory=datetime.now)
    hit_count: int = 0
    last_accessed: datetime = field(default_factory=datetime.now)
    ttl_seconds: int | None = None

    def is_expired(self) -> bool:
        """Check if this entry has expired."""
        if self.ttl_seconds is None:
            return False
        return datetime.now() > self.created_at + timedelta(seconds=self.ttl_seconds)

    def access(self) -> Any:
        """Record an access and return the value."""
        self.hit_count += 1
        self.last_accessed = datetime.now()
        return self.value


class SynthesisCache:
    """Cache for synthesis operations.

    Features:
    - Content-addressed storage (hash-based keys)
    - TTL-based expiration
    - LRU eviction when max size reached
    - Statistics tracking
    """

    def __init__(self, max_size: int = 10000, default_ttl: int | None = 3600):
        """Initialize cache.

        Args:
            max_size: Maximum number of entries
            default_ttl: Default TTL in seconds (None for no expiration)
        """
        self._cache: dict[str, CacheEntry] = {}
        self._max_size = max_size
        self._default_ttl = default_ttl

        # Statistics
        self._hits = 0
        self._misses = 0

    def _make_key(self, *parts: Any) -> str:
        """Create a cache key from parts."""
        serialized = json.dumps(parts, sort_keys=True, default=str)
        return hashlib.sha256(serialized.encode()).hexdigest()[:16]

    def get(self, key: str) -> Any | None:
        """Get a cached value.

        Returns None if not found or expired.
        """
        entry = self._cache.get(key)

        if entry is None:
            self._misses += 1
            return None

        if entry.is_expired():
            del self._cache[key]
            self._misses += 1
            return None

        self._hits += 1
        return entry.access()

    def set(
        self,
        key: str,
        value: Any,
        ttl: int | None = None,
    ) -> None:
        """Store a value in the cache.

        Args:
            key: Cache key
            value: Value to store
            ttl: TTL in seconds (None uses default)
        """
        # Evict if needed
        if len(self._cache) >= self._max_size:
            self._evict_lru()

        self._cache[key] = CacheEntry(
            value=value,
            ttl_seconds=ttl if ttl is not None else self._default_ttl,
        )

    def _evict_lru(self) -> None:
        """Evict least recently used entries."""
        # Remove expired entries first
        expired = [k for k, v in self._cache.items() if v.is_expired()]
        for k in expired:
            del self._cache[k]

        # If still over capacity, remove LRU entries
        if len(self._cache) >= self._max_size:
            # Sort by last_accessed, remove oldest quarter
            sorted_keys = sorted(
                self._cache.keys(),
                key=lambda k: self._cache[k].last_accessed,
            )
            num_to_remove = len(self._cache) // 4 + 1
            for k in sorted_keys[:num_to_remove]:
                del self._cache[k]

    def clear(self) -> None:
        """Clear all cached entries."""
        self._cache.clear()
        self._hits = 0
        self._misses = 0

    @property
    def hit_rate(self) -> float:
        """Get cache hit rate."""
        total = self._hits + self._misses
        return self._hits / total if total > 0 else 0.0

    @property
    def stats(self) -> dict[str, Any]:
        """Get cache statistics."""
        return {
            "size": len(self._cache),
            "max_size": self._max_size,
            "hits": self._hits,
            "misses": self._misses,
            "hit_rate": self.hit_rate,
        }


class ExecutionResultCache(SynthesisCache):
    """Specialized cache for test execution results.

    Keys are based on:
    - Test code/specification
    - Solution being tested
    - Python version (optional)
    """

    def cache_key(
        self,
        test: str,
        solution: str,
        python_version: str | None = None,
    ) -> str:
        """Create a cache key for a test execution."""
        return self._make_key(test, solution, python_version)

    def get_test_result(
        self,
        test: str,
        solution: str,
        python_version: str | None = None,
    ) -> bool | None:
        """Get cached test result.

        Returns:
            True if test passed, False if failed, None if not cached.
        """
        key = self.cache_key(test, solution, python_version)
        return self.get(key)

    def cache_test_result(
        self,
        test: str,
        solution: str,
        passed: bool,
        python_version: str | None = None,
        ttl: int | None = None,
    ) -> None:
        """Cache a test execution result."""
        key = self.cache_key(test, solution, python_version)
        self.set(key, passed, ttl)


class SolutionCache(SynthesisCache):
    """Specialized cache for subproblem solutions.

    Keys are based on:
    - Specification description
    - Type signature
    - Context hash
    """

    def cache_key(self, spec: Specification, context_hash: str | None = None) -> str:
        """Create a cache key for a subproblem solution."""
        return self._make_key(
            spec.description,
            spec.type_signature,
            tuple(spec.constraints),
            context_hash,
        )

    def get_solution(
        self,
        spec: Specification,
        context_hash: str | None = None,
    ) -> Any | None:
        """Get cached solution for a subproblem."""
        key = self.cache_key(spec, context_hash)
        return self.get(key)

    def cache_solution(
        self,
        spec: Specification,
        solution: Any,
        context_hash: str | None = None,
        ttl: int | None = None,
    ) -> None:
        """Cache a subproblem solution."""
        key = self.cache_key(spec, context_hash)
        self.set(key, solution, ttl)


class StrategyCache(SynthesisCache):
    """Cache for strategy selection results."""

    def cache_key(self, spec: Specification) -> str:
        """Create a cache key for strategy selection."""
        return self._make_key(
            spec.description,
            spec.type_signature,
            tuple(spec.constraints),
        )

    def get_strategy(self, spec: Specification) -> str | None:
        """Get cached strategy name for a specification."""
        key = self.cache_key(spec)
        return self.get(key)

    def cache_strategy(
        self,
        spec: Specification,
        strategy_name: str,
        ttl: int = 600,  # Short TTL since context matters
    ) -> None:
        """Cache a strategy selection."""
        key = self.cache_key(spec)
        self.set(key, strategy_name, ttl)


# Global cache instances
_test_cache: ExecutionResultCache | None = None
_solution_cache: SolutionCache | None = None
_strategy_cache: StrategyCache | None = None


def get_test_cache() -> ExecutionResultCache:
    """Get or create the global test result cache."""
    global _test_cache
    if _test_cache is None:
        _test_cache = ExecutionResultCache(max_size=50000, default_ttl=3600)
    return _test_cache


def get_solution_cache() -> SolutionCache:
    """Get or create the global solution cache."""
    global _solution_cache
    if _solution_cache is None:
        _solution_cache = SolutionCache(max_size=10000, default_ttl=7200)
    return _solution_cache


def get_strategy_cache() -> StrategyCache:
    """Get or create the global strategy cache."""
    global _strategy_cache
    if _strategy_cache is None:
        _strategy_cache = StrategyCache(max_size=5000, default_ttl=600)
    return _strategy_cache


def clear_all_caches() -> None:
    """Clear all synthesis caches."""
    if _test_cache:
        _test_cache.clear()
    if _solution_cache:
        _solution_cache.clear()
    if _strategy_cache:
        _strategy_cache.clear()


def get_cache_stats() -> dict[str, dict[str, Any]]:
    """Get statistics for all caches."""
    return {
        "test_cache": get_test_cache().stats,
        "solution_cache": get_solution_cache().stats,
        "strategy_cache": get_strategy_cache().stats,
    }
