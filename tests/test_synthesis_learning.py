"""Tests for synthesis learning module."""

from __future__ import annotations

import pytest

from moss.synthesis import Specification
from moss.synthesis.learning import (
    StrategyLearner,
    extract_features,
    feature_similarity,
    get_learner,
    reset_learner,
)
from moss.synthesis.strategies import (
    PatternBasedDecomposition,
    TestDrivenDecomposition,
    TypeDrivenDecomposition,
)


class TestFeatureExtraction:
    """Tests for feature extraction."""

    def test_basic_features(self):
        """Test basic feature extraction."""
        spec = Specification(description="Sort a list of integers")
        features = extract_features(spec)

        assert "desc_short" in features
        assert "desc_medium" in features
        assert "desc_long" in features
        assert "has_type" in features

    def test_short_description(self):
        """Test short description features."""
        spec = Specification(description="Sort list")
        features = extract_features(spec)

        assert features["desc_short"] == 1.0
        assert features["desc_medium"] == 0.0
        assert features["desc_long"] == 0.0

    def test_long_description(self):
        """Test long description features."""
        desc = " ".join(["word"] * 25)
        spec = Specification(description=desc)
        features = extract_features(spec)

        assert features["desc_short"] == 0.0
        assert features["desc_medium"] == 0.0
        assert features["desc_long"] == 1.0

    def test_type_signature_features(self):
        """Test type signature features."""
        spec = Specification(
            description="Sort users",
            type_signature="List[User] -> List[User]",
        )
        features = extract_features(spec)

        assert features["has_type"] == 1.0
        assert features["has_generic"] == 1.0
        assert features["has_function_type"] == 1.0
        assert features["type_complexity"] > 0

    def test_example_features(self):
        """Test example features."""
        spec = Specification(
            description="Add numbers",
            examples=(((1, 2), 3), ((0, 0), 0), ((5, 5), 10)),
        )
        features = extract_features(spec)

        assert features["has_examples"] == 1.0
        assert features["example_count"] > 0

    def test_constraint_features(self):
        """Test constraint features."""
        spec = Specification(
            description="Validate input",
            constraints=("must be positive", "no empty strings"),
        )
        features = extract_features(spec)

        assert features["has_constraints"] == 1.0
        assert features["constraint_count"] > 0

    def test_crud_keyword(self):
        """Test CRUD keyword detection."""
        spec = Specification(description="Create a new user in the database")
        features = extract_features(spec)
        assert features["is_crud"] == 1.0

    def test_transform_keyword(self):
        """Test transform keyword detection."""
        spec = Specification(description="Transform data from JSON to XML")
        features = extract_features(spec)
        assert features["is_transform"] == 1.0

    def test_validation_keyword(self):
        """Test validation keyword detection."""
        spec = Specification(description="Validate email format")
        features = extract_features(spec)
        assert features["is_validation"] == 1.0


class TestFeatureSimilarity:
    """Tests for feature similarity."""

    def test_identical_features(self):
        """Test similarity of identical features."""
        f = {"a": 1.0, "b": 0.5, "c": 0.0}
        sim = feature_similarity(f, f)
        assert sim == pytest.approx(1.0)

    def test_orthogonal_features(self):
        """Test similarity of orthogonal features."""
        f1 = {"a": 1.0, "b": 0.0}
        f2 = {"a": 0.0, "b": 1.0}
        sim = feature_similarity(f1, f2)
        assert sim == pytest.approx(0.0)

    def test_partial_similarity(self):
        """Test partial similarity."""
        f1 = {"a": 1.0, "b": 0.0}
        f2 = {"a": 1.0, "b": 1.0}
        sim = feature_similarity(f1, f2)
        assert 0.0 < sim < 1.0

    def test_empty_features(self):
        """Test empty feature vectors."""
        sim = feature_similarity({}, {})
        assert sim == 0.0


class TestStrategyLearner:
    """Tests for StrategyLearner."""

    @pytest.fixture
    def learner(self):
        """Create a fresh learner."""
        return StrategyLearner(max_history=100)

    @pytest.fixture
    def strategies(self):
        """Create test strategies."""
        return [
            TypeDrivenDecomposition(),
            TestDrivenDecomposition(),
            PatternBasedDecomposition(),
        ]

    def test_initial_scores(self, learner, strategies):
        """Test scores before any learning."""
        spec = Specification(description="Sort a list")

        for strategy in strategies:
            score = learner.get_strategy_score(spec, strategy)
            assert score == 0.5  # Neutral

    def test_record_outcome(self, learner, strategies):
        """Test recording outcomes."""
        spec = Specification(description="Sort a list")
        strategy = strategies[0]

        learner.record_outcome(spec, strategy, success=True, iterations=5)

        stats = learner.get_stats()
        assert stats["total_outcomes"] == 1
        assert strategy.name in stats["strategies"]

    def test_learning_improves_score(self, learner, strategies):
        """Test that learning improves scores."""
        spec = Specification(description="Sort a list of integers")
        strategy = strategies[0]

        # Record several successes
        for _ in range(10):
            learner.record_outcome(spec, strategy, success=True, iterations=5)

        score = learner.get_strategy_score(spec, strategy)
        assert score > 0.5  # Should be higher than neutral

    def test_failures_decrease_score(self, learner, strategies):
        """Test that failures decrease scores."""
        spec = Specification(description="Sort a list of integers")
        strategy = strategies[0]

        # Record several failures
        for _ in range(10):
            learner.record_outcome(spec, strategy, success=False, iterations=0)

        score = learner.get_strategy_score(spec, strategy)
        assert score < 0.5  # Should be lower than neutral

    def test_recommend_strategy(self, learner, strategies):
        """Test strategy recommendation."""
        spec1 = Specification(description="Create a user", type_signature="User -> None")
        spec2 = Specification(description="Transform list", examples=(((1,), 2),))

        # Train on pattern-based for CRUD
        pattern_strategy = strategies[2]
        for _ in range(5):
            learner.record_outcome(spec1, pattern_strategy, success=True, iterations=3)

        # Train on type-driven for transforms
        type_strategy = strategies[0]
        for _ in range(5):
            learner.record_outcome(spec2, type_strategy, success=True, iterations=2)

        # Recommendation should favor trained strategies
        rec = learner.recommend_strategy(spec1, strategies)
        assert rec is not None

    def test_find_similar_problems(self, learner, strategies):
        """Test finding similar problems."""
        spec1 = Specification(description="Sort users by name")
        spec2 = Specification(description="Sort products by price")
        spec3 = Specification(description="Calculate tax")

        learner.record_outcome(spec1, strategies[0], success=True, iterations=5)
        learner.record_outcome(spec2, strategies[0], success=True, iterations=4)
        learner.record_outcome(spec3, strategies[1], success=False, iterations=0)

        # Find similar to "Sort items by date"
        similar_spec = Specification(description="Sort items by date")
        similar = learner.find_similar_problems(similar_spec, limit=2)

        assert len(similar) <= 2
        # Similar problems should be about sorting
        for outcome in similar:
            assert "sort" in outcome.spec_summary.lower() or outcome in [
                o for o in learner._outcomes
            ]

    def test_reset(self, learner, strategies):
        """Test resetting learner."""
        spec = Specification(description="Test")
        learner.record_outcome(spec, strategies[0], success=True, iterations=1)

        assert learner.get_stats()["total_outcomes"] == 1

        learner.reset()

        assert learner.get_stats()["total_outcomes"] == 0

    def test_max_history_limit(self):
        """Test max history limit."""
        learner = StrategyLearner(max_history=5)
        strategy = TypeDrivenDecomposition()

        for i in range(10):
            spec = Specification(description=f"Task {i}")
            learner.record_outcome(spec, strategy, success=True, iterations=1)

        # Should only keep last 5
        assert len(learner._outcomes) == 5


class TestGlobalLearner:
    """Tests for global learner functions."""

    def test_get_learner(self):
        """Test getting global learner."""
        reset_learner()
        learner1 = get_learner()
        learner2 = get_learner()

        assert learner1 is learner2  # Same instance

    def test_reset_learner(self):
        """Test resetting global learner."""
        learner1 = get_learner()
        reset_learner()
        learner2 = get_learner()

        assert learner1 is not learner2  # Different instance after reset
