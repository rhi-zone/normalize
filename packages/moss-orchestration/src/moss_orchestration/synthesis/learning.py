"""Memory-based strategy learning for synthesis.

This module provides learning mechanisms that improve strategy selection
based on historical outcomes. It tracks:
- Strategy success rates per problem type
- Pattern extraction from successful solutions
- Feature importance for strategy selection

Usage:
    from moss_orchestration.synthesis.learning import StrategyLearner

    learner = StrategyLearner()
    learner.record_outcome(spec, strategy, success, iterations)
    best_strategy = learner.recommend_strategy(spec, strategies)
"""

from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .strategy import DecompositionStrategy
    from .types import Specification


# =============================================================================
# Feature Extraction
# =============================================================================


def extract_features(spec: Specification) -> dict[str, float]:
    """Extract features from a specification for learning.

    Features capture problem characteristics that affect strategy selection:
    - Length: short problems vs complex descriptions
    - Type complexity: presence/complexity of type signatures
    - Example count: how many examples provided
    - Constraint count: number of constraints
    - Keywords: presence of key patterns

    Args:
        spec: The specification to extract features from

    Returns:
        Dictionary of feature name to value
    """
    features: dict[str, float] = {}

    # Length features
    desc_words = len(spec.description.split())
    features["desc_short"] = 1.0 if desc_words <= 5 else 0.0
    features["desc_medium"] = 1.0 if 5 < desc_words <= 20 else 0.0
    features["desc_long"] = 1.0 if desc_words > 20 else 0.0

    # Type signature features
    features["has_type"] = 1.0 if spec.type_signature else 0.0
    if spec.type_signature:
        # Count type complexity
        type_parts = re.findall(r"[A-Z]\w+", spec.type_signature)
        features["type_complexity"] = min(len(type_parts) / 5.0, 1.0)
        features["has_generic"] = 1.0 if "[" in spec.type_signature else 0.0
        features["has_function_type"] = 1.0 if "->" in spec.type_signature else 0.0
    else:
        features["type_complexity"] = 0.0
        features["has_generic"] = 0.0
        features["has_function_type"] = 0.0

    # Example features
    features["has_examples"] = 1.0 if spec.examples else 0.0
    features["example_count"] = min(len(spec.examples) / 5.0, 1.0) if spec.examples else 0.0

    # Constraint features
    features["has_constraints"] = 1.0 if spec.constraints else 0.0
    if spec.constraints:
        features["constraint_count"] = min(len(spec.constraints) / 5.0, 1.0)
    else:
        features["constraint_count"] = 0.0

    # Keyword features (indicate problem type)
    desc_lower = spec.description.lower()

    # CRUD patterns
    crud_keywords = ["create", "read", "update", "delete", "crud", "add", "remove", "get", "set"]
    features["is_crud"] = 1.0 if any(kw in desc_lower for kw in crud_keywords) else 0.0

    # Transformation patterns
    transform_keywords = ["transform", "convert", "map", "filter", "reduce", "sort", "parse"]
    features["is_transform"] = 1.0 if any(kw in desc_lower for kw in transform_keywords) else 0.0

    # Validation patterns
    validation_keywords = ["validate", "check", "verify", "ensure", "assert", "test"]
    features["is_validation"] = 1.0 if any(kw in desc_lower for kw in validation_keywords) else 0.0

    # API patterns
    api_keywords = ["api", "endpoint", "request", "response", "http", "rest"]
    features["is_api"] = 1.0 if any(kw in desc_lower for kw in api_keywords) else 0.0

    # Recursive patterns
    recursive_keywords = ["recursive", "tree", "traverse", "nested", "deep"]
    features["is_recursive"] = 1.0 if any(kw in desc_lower for kw in recursive_keywords) else 0.0

    return features


def feature_similarity(f1: dict[str, float], f2: dict[str, float]) -> float:
    """Calculate cosine similarity between two feature vectors.

    Args:
        f1: First feature vector
        f2: Second feature vector

    Returns:
        Similarity score between 0 and 1
    """
    # Get all keys
    all_keys = set(f1.keys()) | set(f2.keys())

    if not all_keys:
        return 0.0

    # Calculate dot product and magnitudes
    dot_product = 0.0
    mag1 = 0.0
    mag2 = 0.0

    for key in all_keys:
        v1 = f1.get(key, 0.0)
        v2 = f2.get(key, 0.0)
        dot_product += v1 * v2
        mag1 += v1 * v1
        mag2 += v2 * v2

    if mag1 == 0 or mag2 == 0:
        return 0.0

    return dot_product / (mag1**0.5 * mag2**0.5)


# =============================================================================
# Outcome Tracking
# =============================================================================


@dataclass
class StrategyOutcome:
    """Record of a synthesis attempt outcome."""

    strategy_name: str
    features: dict[str, float]
    success: bool
    iterations: int
    spec_summary: str


@dataclass
class StrategyStats:
    """Aggregated statistics for a strategy."""

    total_attempts: int = 0
    successes: int = 0
    total_iterations: int = 0
    feature_weights: dict[str, float] = field(default_factory=dict)

    @property
    def success_rate(self) -> float:
        """Calculate success rate."""
        if self.total_attempts == 0:
            return 0.5  # Neutral if no data
        return self.successes / self.total_attempts

    @property
    def avg_iterations(self) -> float:
        """Calculate average iterations."""
        if self.successes == 0:
            return 0.0
        return self.total_iterations / self.successes


# =============================================================================
# Strategy Learner
# =============================================================================


class StrategyLearner:
    """Learn strategy preferences from synthesis outcomes.

    This class implements memory-based learning for strategy selection:
    1. Tracks outcomes per strategy
    2. Extracts features from specifications
    3. Learns feature-strategy correlations
    4. Recommends strategies based on learned patterns

    The learning is incremental - each outcome updates the model.
    """

    def __init__(self, max_history: int = 1000):
        """Initialize the learner.

        Args:
            max_history: Maximum number of outcomes to retain
        """
        self.max_history = max_history
        self._outcomes: list[StrategyOutcome] = []
        self._stats: dict[str, StrategyStats] = defaultdict(StrategyStats)
        self._feature_strategy_scores: dict[str, dict[str, float]] = defaultdict(
            lambda: defaultdict(float)
        )

    def record_outcome(
        self,
        spec: Specification,
        strategy: DecompositionStrategy,
        success: bool,
        iterations: int = 0,
    ) -> None:
        """Record the outcome of a synthesis attempt.

        Args:
            spec: The specification that was synthesized
            strategy: The strategy that was used
            success: Whether synthesis succeeded
            iterations: Number of iterations taken
        """
        # Extract features
        features = extract_features(spec)

        # Create outcome record
        outcome = StrategyOutcome(
            strategy_name=strategy.name,
            features=features,
            success=success,
            iterations=iterations,
            spec_summary=spec.summary(),
        )

        # Store outcome (with size limit)
        self._outcomes.append(outcome)
        if len(self._outcomes) > self.max_history:
            self._outcomes.pop(0)

        # Update statistics
        stats = self._stats[strategy.name]
        stats.total_attempts += 1
        if success:
            stats.successes += 1
            stats.total_iterations += iterations

        # Update feature-strategy correlations
        self._update_feature_weights(strategy.name, features, success)

    def _update_feature_weights(
        self,
        strategy_name: str,
        features: dict[str, float],
        success: bool,
    ) -> None:
        """Update feature weights based on outcome.

        Uses exponential moving average to update weights.
        """
        alpha = 0.1  # Learning rate

        for feature_name, feature_value in features.items():
            if feature_value == 0:
                continue

            current = self._feature_strategy_scores[feature_name].get(strategy_name, 0.5)
            target = 1.0 if success else 0.0

            # Update using EMA
            new_value = current + alpha * (target - current)
            self._feature_strategy_scores[feature_name][strategy_name] = new_value

    def get_strategy_score(
        self,
        spec: Specification,
        strategy: DecompositionStrategy,
    ) -> float:
        """Get a learned score for a strategy on a specification.

        Args:
            spec: The specification
            strategy: The strategy to score

        Returns:
            Score between 0 and 1 (higher is better)
        """
        features = extract_features(spec)
        strategy_name = strategy.name

        # Base score from overall success rate
        stats = self._stats.get(strategy_name)
        if stats is None:
            return 0.5  # Neutral for unknown strategies

        base_score = stats.success_rate

        # Adjust based on feature correlations
        feature_adjustment = 0.0
        active_features = 0

        for feature_name, feature_value in features.items():
            if feature_value == 0:
                continue

            score = self._feature_strategy_scores[feature_name].get(strategy_name, 0.5)
            feature_adjustment += (score - 0.5) * feature_value
            active_features += 1

        # Normalize adjustment
        if active_features > 0:
            feature_adjustment /= active_features

        # Combine base score with feature adjustment
        final_score = base_score + feature_adjustment * 0.3  # 30% weight for features
        return max(0.0, min(1.0, final_score))

    def recommend_strategy(
        self,
        spec: Specification,
        strategies: list[DecompositionStrategy],
    ) -> DecompositionStrategy | None:
        """Recommend the best strategy based on learned patterns.

        Args:
            spec: The specification to synthesize
            strategies: Available strategies

        Returns:
            Best strategy, or None if no recommendation
        """
        if not strategies:
            return None

        # Score all strategies
        scored = [(s, self.get_strategy_score(spec, s)) for s in strategies]

        # Sort by score descending
        scored.sort(key=lambda x: x[1], reverse=True)

        return scored[0][0]

    def find_similar_problems(
        self,
        spec: Specification,
        limit: int = 5,
    ) -> list[StrategyOutcome]:
        """Find similar problems from history.

        Args:
            spec: The specification to find similar problems for
            limit: Maximum number of results

        Returns:
            List of similar outcomes
        """
        features = extract_features(spec)

        # Score all outcomes by similarity
        scored = []
        for outcome in self._outcomes:
            similarity = feature_similarity(features, outcome.features)
            scored.append((outcome, similarity))

        # Sort by similarity descending
        scored.sort(key=lambda x: x[1], reverse=True)

        return [outcome for outcome, _ in scored[:limit]]

    def get_stats(self) -> dict[str, dict]:
        """Get learning statistics.

        Returns:
            Dictionary with strategy stats and overall metrics
        """
        return {
            "total_outcomes": len(self._outcomes),
            "strategies": {
                name: {
                    "attempts": stats.total_attempts,
                    "successes": stats.successes,
                    "success_rate": stats.success_rate,
                    "avg_iterations": stats.avg_iterations,
                }
                for name, stats in self._stats.items()
            },
            "feature_weights": dict(self._feature_strategy_scores),
        }

    def reset(self) -> None:
        """Reset all learning data."""
        self._outcomes.clear()
        self._stats.clear()
        self._feature_strategy_scores.clear()


# Global learner instance
_learner: StrategyLearner | None = None


def get_learner() -> StrategyLearner:
    """Get the global strategy learner instance."""
    global _learner
    if _learner is None:
        _learner = StrategyLearner()
    return _learner


def reset_learner() -> None:
    """Reset the global learner."""
    global _learner
    _learner = None


__all__ = [
    "StrategyLearner",
    "StrategyOutcome",
    "StrategyStats",
    "extract_features",
    "feature_similarity",
    "get_learner",
    "reset_learner",
]
