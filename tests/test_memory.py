"""Tests for Memory Layer."""

from typing import Any, Literal

import pytest

from moss.memory import (
    Action,
    Episode,
    EpisodicStore,
    MemoryLayer,
    MemoryManager,
    MemoryPlugin,
    Outcome,
    PatternMatcher,
    SemanticRule,
    SemanticStore,
    SimpleVectorIndex,
    StateSnapshot,
    create_memory_manager,
    discover_plugins,
)


class TestStateSnapshot:
    """Tests for StateSnapshot."""

    def test_create_snapshot(self):
        snapshot = StateSnapshot.create(
            files=["src/main.py", "tests/test_main.py"],
            context="def hello(): pass",
            error_count=0,
        )

        assert snapshot.files == frozenset(["src/main.py", "tests/test_main.py"])
        assert len(snapshot.context_hash) == 16
        assert snapshot.error_count == 0

    def test_snapshot_with_metadata(self):
        snapshot = StateSnapshot.create(
            files=["a.py"],
            context="x = 1",
            metadata={"branch": "main", "commit": "abc123"},
        )

        assert ("branch", "main") in snapshot.metadata
        assert ("commit", "abc123") in snapshot.metadata


class TestAction:
    """Tests for Action."""

    def test_create_action(self):
        action = Action.create(
            tool="edit",
            target="src/main.py",
            description="Add function",
            content="new code",
        )

        assert action.tool == "edit"
        assert action.target == "src/main.py"
        assert ("content", "new code") in action.parameters

    def test_action_without_target(self):
        action = Action.create(tool="shell", description="Run tests")
        assert action.target is None


class TestEpisode:
    """Tests for Episode."""

    def test_create_episode(self):
        state = StateSnapshot.create(files=["a.py"], context="code")
        action = Action.create(tool="edit", target="a.py")

        episode = Episode.create(
            state=state,
            action=action,
            outcome=Outcome.SUCCESS,
            duration_ms=100,
        )

        assert len(episode.id) == 12
        assert episode.outcome == Outcome.SUCCESS
        assert episode.duration_ms == 100

    def test_episode_with_error(self):
        state = StateSnapshot.create(files=["a.py"], context="code")
        action = Action.create(tool="edit", target="a.py")

        episode = Episode.create(
            state=state,
            action=action,
            outcome=Outcome.FAILURE,
            error_message="Syntax error at line 5",
        )

        assert episode.outcome == Outcome.FAILURE
        assert episode.error_message == "Syntax error at line 5"

    def test_episode_with_tags(self):
        state = StateSnapshot.create(files=["a.py"], context="code")
        action = Action.create(tool="edit", target="a.py")

        episode = Episode.create(
            state=state,
            action=action,
            outcome=Outcome.SUCCESS,
            tags={"refactor", "python"},
        )

        assert "refactor" in episode.tags
        assert "python" in episode.tags


class TestSimpleVectorIndex:
    """Tests for SimpleVectorIndex."""

    @pytest.fixture
    def index(self):
        return SimpleVectorIndex()

    async def test_index_and_search(self, index: SimpleVectorIndex):
        await index.index("1", "python code function", {"type": "code"})
        await index.index("2", "python test unittest", {"type": "test"})
        await index.index("3", "javascript react component", {"type": "code"})

        results = await index.search("python function")

        assert len(results) > 0
        # First result should be the python code (more overlap)
        assert results[0][0] == "1"

    async def test_search_with_filter(self, index: SimpleVectorIndex):
        await index.index("1", "python code", {"type": "code"})
        await index.index("2", "python test", {"type": "test"})

        results = await index.search("python", filter={"type": "test"})

        assert len(results) == 1
        assert results[0][0] == "2"

    async def test_delete(self, index: SimpleVectorIndex):
        await index.index("1", "content", {})

        assert await index.delete("1")
        assert not await index.delete("1")  # Already deleted

        results = await index.search("content")
        assert len(results) == 0


class TestEpisodicStore:
    """Tests for EpisodicStore."""

    @pytest.fixture
    def store(self):
        return EpisodicStore()

    @pytest.fixture
    def sample_episode(self):
        state = StateSnapshot.create(files=["src/main.py"], context="def main(): pass")
        action = Action.create(tool="edit", target="src/main.py")
        return Episode.create(state=state, action=action, outcome=Outcome.SUCCESS)

    async def test_store_and_get(self, store: EpisodicStore, sample_episode: Episode):
        id = await store.store(sample_episode)

        retrieved = await store.get(id)
        assert retrieved is not None
        assert retrieved.id == sample_episode.id

    async def test_delete(self, store: EpisodicStore, sample_episode: Episode):
        id = await store.store(sample_episode)

        assert await store.delete(id)
        assert await store.get(id) is None

    async def test_find_similar(self, store: EpisodicStore):
        # Store some episodes
        for i in range(5):
            state = StateSnapshot.create(files=[f"src/module{i}.py"], context=f"code {i}")
            action = Action.create(tool="edit", target=f"src/module{i}.py")
            episode = Episode.create(state=state, action=action, outcome=Outcome.SUCCESS)
            await store.store(episode)

        # Search
        query_state = StateSnapshot.create(files=["src/module0.py"], context="query")
        query_action = Action.create(tool="edit", target="src/module0.py")

        results = await store.find_similar(query_state, query_action, limit=3)

        assert len(results) <= 3

    async def test_find_failures(self, store: EpisodicStore):
        # Store success and failure episodes
        for outcome in [Outcome.SUCCESS, Outcome.FAILURE, Outcome.FAILURE]:
            state = StateSnapshot.create(files=["a.py"], context="code")
            action = Action.create(tool="edit", target="a.py")
            episode = Episode.create(state=state, action=action, outcome=outcome)
            await store.store(episode)

        failures = await store.find_failures()

        assert len(failures) == 2
        assert all(ep.outcome == Outcome.FAILURE for ep in failures)

    async def test_find_by_tag(self, store: EpisodicStore):
        state = StateSnapshot.create(files=["a.py"], context="code")
        action = Action.create(tool="edit", target="a.py")
        episode = Episode.create(
            state=state, action=action, outcome=Outcome.SUCCESS, tags={"important"}
        )
        await store.store(episode)

        results = await store.find_by_tag("important")

        assert len(results) == 1
        assert "important" in results[0].tags

    async def test_stats(self, store: EpisodicStore, sample_episode: Episode):
        await store.store(sample_episode)

        stats = store.stats()

        assert stats["total"] == 1
        assert stats["by_outcome"]["SUCCESS"] == 1
        assert "edit" in stats["by_tool"]

    async def test_max_episodes_eviction(self):
        store = EpisodicStore(max_episodes=3)

        # Store 5 episodes
        for i in range(5):
            state = StateSnapshot.create(files=[f"file{i}.py"], context=f"code{i}")
            action = Action.create(tool="edit", target=f"file{i}.py")
            episode = Episode.create(state=state, action=action, outcome=Outcome.SUCCESS)
            await store.store(episode)

        assert store.count == 3  # Only 3 remain


class TestSemanticRule:
    """Tests for SemanticRule."""

    def test_matches_pattern(self):
        rule = SemanticRule(
            id="test",
            pattern="python syntax error",
            action="Check syntax",
            confidence=0.8,
            supporting_episodes=[],
        )

        assert rule.matches("There was a python syntax error in the file")
        assert not rule.matches("JavaScript runtime error")

    def test_record_match(self):
        rule = SemanticRule(
            id="test",
            pattern="test",
            action="action",
            confidence=0.8,
            supporting_episodes=[],
        )

        assert rule.match_count == 0
        rule.record_match()
        assert rule.match_count == 1
        assert rule.last_matched is not None


class TestSemanticStore:
    """Tests for SemanticStore."""

    @pytest.fixture
    def store(self):
        return SemanticStore()

    def test_add_and_get_rule(self, store: SemanticStore):
        rule = SemanticRule(
            id="rule1",
            pattern="edit python file",
            action="Run linter",
            confidence=0.9,
            supporting_episodes=[],
        )

        store.add_rule(rule)

        retrieved = store.get_rule("rule1")
        assert retrieved is not None
        assert retrieved.pattern == "edit python file"

    def test_remove_rule(self, store: SemanticStore):
        rule = SemanticRule(
            id="rule1",
            pattern="test",
            action="action",
            confidence=0.8,
            supporting_episodes=[],
        )
        store.add_rule(rule)

        assert store.remove_rule("rule1")
        assert store.get_rule("rule1") is None

    def test_find_matching_rules(self, store: SemanticStore):
        rule1 = SemanticRule(
            id="r1",
            pattern="python syntax",
            action="Run syntax check",
            confidence=0.9,
            supporting_episodes=[],
        )
        rule2 = SemanticRule(
            id="r2",
            pattern="javascript",
            action="Run eslint",
            confidence=0.8,
            supporting_episodes=[],
        )
        store.add_rule(rule1)
        store.add_rule(rule2)

        matches = store.find_matching_rules("python syntax error in file")

        assert len(matches) == 1
        assert matches[0].id == "r1"

    def test_min_confidence_filter(self, store: SemanticStore):
        rule = SemanticRule(
            id="r1",
            pattern="test",
            action="action",
            confidence=0.4,  # Low confidence
            supporting_episodes=[],
        )
        store.add_rule(rule)

        # Default min_confidence is 0.5
        matches = store.find_matching_rules("test context")
        assert len(matches) == 0

        # Lower threshold
        matches = store.find_matching_rules("test context", min_confidence=0.3)
        assert len(matches) == 1


class TestPatternMatcher:
    """Tests for PatternMatcher."""

    @pytest.fixture
    def stores(self):
        return EpisodicStore(), SemanticStore()

    @pytest.fixture
    def matcher(self, stores):
        episodic, semantic = stores
        return PatternMatcher(episodic, semantic, min_occurrences=2, min_confidence=0.5)

    async def test_analyze_failures_creates_rules(self, stores, matcher: PatternMatcher):
        episodic, semantic = stores

        # Create multiple failures with same pattern
        for i in range(3):
            state = StateSnapshot.create(files=["src/main.py"], context=f"code{i}")
            action = Action.create(tool="edit", target="src/main.py")
            episode = Episode.create(
                state=state,
                action=action,
                outcome=Outcome.FAILURE,
                error_message="Syntax error",
            )
            await episodic.store(episode)

        new_rules = await matcher.analyze_failures()

        assert len(new_rules) >= 1
        assert len(semantic.rules) >= 1


class TestMemoryManager:
    """Tests for MemoryManager."""

    @pytest.fixture
    def manager(self):
        return create_memory_manager()

    async def test_record_episode(self, manager: MemoryManager):
        state = StateSnapshot.create(files=["a.py"], context="code")
        action = Action.create(tool="edit", target="a.py")

        episode = await manager.record_episode(
            state=state,
            action=action,
            outcome=Outcome.SUCCESS,
            duration_ms=50,
        )

        assert episode.id is not None
        assert manager.episodic.count == 1

    async def test_get_context(self, manager: MemoryManager):
        # Record some episodes first
        state = StateSnapshot.create(files=["src/main.py"], context="code")
        action = Action.create(tool="edit", target="src/main.py")

        await manager.record_episode(
            state=state, action=action, outcome=Outcome.FAILURE, error_message="Error"
        )
        await manager.record_episode(
            state=state, action=action, outcome=Outcome.FAILURE, error_message="Error"
        )

        # Get context for similar action
        context = await manager.get_context(state, action)

        # Should find the similar episodes
        assert len(context.relevant_episodes) > 0

    async def test_add_manual_rule(self, manager: MemoryManager):
        rule_id = manager.add_rule(pattern="edit python", action="Run ruff first", confidence=0.9)

        rule = manager.semantic.get_rule(rule_id)
        assert rule is not None
        assert rule.pattern == "edit python"

    async def test_context_to_text(self, manager: MemoryManager):
        # Add a rule that will match the action
        manager.add_rule(pattern="edit test", action="Be careful!", confidence=0.8)

        # Get context that matches the rule (search is: "edit test.py test.py")
        state = StateSnapshot.create(files=["test.py"], context="some context")
        action = Action.create(tool="edit", target="test.py")

        context = await manager.get_context(state, action)
        text = context.to_text()

        assert "Relevant learned rules" in text
        assert "Be careful!" in text

    async def test_recall_returns_episodes(self, manager: MemoryManager):
        # Record some episodes - use keywords that will match via word overlap
        state = StateSnapshot.create(files=["auth.py"], context="auth login")
        action = Action.create(tool="edit", target="auth.py", description="Modified auth flow")

        await manager.record_episode(
            state=state, action=action, outcome=Outcome.SUCCESS, duration_ms=50
        )

        # Query uses same keywords (SimpleVectorIndex does word-based matching)
        result = await manager.recall("edit auth.py")

        assert "Past episodes:" in result
        assert "Modified auth flow" in result

    async def test_recall_returns_rules(self, manager: MemoryManager):
        # Pattern must match query words exactly
        manager.add_rule(pattern="auth changes", action="Check permissions first", confidence=0.9)

        result = await manager.recall("auth changes needed")

        assert "Learned patterns:" in result
        assert "Check permissions first" in result

    async def test_recall_no_memories(self, manager: MemoryManager):
        result = await manager.recall("completely unrelated query xyz")

        assert result == "No relevant memories found."


class TestCreateMemoryManager:
    """Tests for create_memory_manager."""

    def test_creates_manager(self):
        manager = create_memory_manager()
        assert manager is not None
        assert manager.episodic is not None
        assert manager.semantic is not None


# =============================================================================
# Plugin System Tests
# =============================================================================


class MockAutomaticPlugin:
    """Mock plugin for automatic layer."""

    name = "mock_automatic"
    layer: Literal["automatic"] = "automatic"

    async def get_context(self, state: StateSnapshot) -> str | None:
        return "Automatic context from mock"

    def configure(self, config: dict[str, Any]) -> None:
        self.config = config


class MockTriggeredPlugin:
    """Mock plugin for triggered layer."""

    name = "mock_triggered"
    layer: Literal["triggered"] = "triggered"

    def __init__(self):
        self.trigger_pattern = "dangerous"

    async def get_context(self, state: StateSnapshot) -> str | None:
        context_str = " ".join(state.files)
        if self.trigger_pattern in context_str:
            return f"Warning: {self.trigger_pattern} detected"
        return None

    def configure(self, config: dict[str, Any]) -> None:
        if "pattern" in config:
            self.trigger_pattern = config["pattern"]


class MockOnDemandPlugin:
    """Mock plugin for on-demand layer."""

    name = "mock_on_demand"
    layer: Literal["on_demand"] = "on_demand"

    async def get_context(self, state: StateSnapshot) -> str | None:
        query = dict(state.metadata).get("query", "")
        if "history" in query:
            return "On-demand history: some past events"
        return None

    def configure(self, config: dict[str, Any]) -> None:
        pass


class TestMemoryPlugin:
    """Tests for MemoryPlugin protocol."""

    def test_mock_plugins_implement_protocol(self):
        auto = MockAutomaticPlugin()
        trig = MockTriggeredPlugin()
        demand = MockOnDemandPlugin()

        assert isinstance(auto, MemoryPlugin)
        assert isinstance(trig, MemoryPlugin)
        assert isinstance(demand, MemoryPlugin)

    def test_plugin_properties(self):
        plugin = MockAutomaticPlugin()

        assert plugin.name == "mock_automatic"
        assert plugin.layer == "automatic"


class TestMemoryLayer:
    """Tests for MemoryLayer."""

    @pytest.fixture
    def layer_with_plugins(self):
        plugins = [
            MockAutomaticPlugin(),
            MockTriggeredPlugin(),
            MockOnDemandPlugin(),
        ]
        return MemoryLayer(plugins=plugins)

    def test_init_with_plugins(self, layer_with_plugins: MemoryLayer):
        assert len(layer_with_plugins.plugins) == 3
        assert layer_with_plugins.manager is not None

    def test_add_plugin(self):
        layer = MemoryLayer()
        plugin = MockAutomaticPlugin()

        layer.add_plugin(plugin)

        assert plugin in layer.plugins

    async def test_get_automatic(self, layer_with_plugins: MemoryLayer):
        result = await layer_with_plugins.get_automatic()

        assert "Automatic context from mock" in result

    async def test_check_triggers_match(self, layer_with_plugins: MemoryLayer):
        state = StateSnapshot.create(files=["dangerous_file.py"], context="code")

        warnings = await layer_with_plugins.check_triggers(state)

        assert len(warnings) == 1
        assert "dangerous" in warnings[0]

    async def test_check_triggers_no_match(self, layer_with_plugins: MemoryLayer):
        state = StateSnapshot.create(files=["safe_file.py"], context="code")

        warnings = await layer_with_plugins.check_triggers(state)

        assert len(warnings) == 0

    async def test_recall_on_demand(self, layer_with_plugins: MemoryLayer):
        result = await layer_with_plugins.recall("show me history")

        assert "On-demand history" in result

    async def test_recall_no_match(self, layer_with_plugins: MemoryLayer):
        result = await layer_with_plugins.recall("unrelated query xyz")

        assert result == "No relevant memories found."

    def test_configure_plugins(self):
        plugin = MockTriggeredPlugin()
        layer = MemoryLayer(plugins=[plugin])

        layer.configure({"mock_triggered": {"pattern": "critical"}})

        assert plugin.trigger_pattern == "critical"

    @pytest.fixture
    def layer_default(self):
        # Create layer without any plugins to test defaults
        return MemoryLayer(plugins=[])

    async def test_empty_layer(self, layer_default: MemoryLayer):
        result = await layer_default.get_automatic()
        assert result == ""

        state = StateSnapshot.create(files=["a.py"], context="")
        warnings = await layer_default.check_triggers(state)
        assert warnings == []


class TestDiscoverPlugins:
    """Tests for discover_plugins function."""

    def test_discover_from_nonexistent_dir(self, tmp_path):
        # Should not fail when directories don't exist
        plugins = discover_plugins(tmp_path)
        assert plugins == []

    def test_discover_from_empty_dir(self, tmp_path):
        (tmp_path / ".moss" / "memory").mkdir(parents=True)
        plugins = discover_plugins(tmp_path)
        assert plugins == []

    def test_discover_skips_underscore_files(self, tmp_path):
        memory_dir = tmp_path / ".moss" / "memory"
        memory_dir.mkdir(parents=True)

        # Create a file starting with underscore
        (memory_dir / "_private.py").write_text("# should be skipped")

        plugins = discover_plugins(tmp_path)
        assert plugins == []

    def test_discover_loads_valid_plugin(self, tmp_path):
        memory_dir = tmp_path / ".moss" / "memory"
        memory_dir.mkdir(parents=True)

        # Create a valid plugin file
        plugin_code = """
from typing import Any, Literal

class TestDiscoveryPlugin:
    name = "test_discovery"
    layer: Literal["automatic"] = "automatic"

    async def get_context(self, state):
        return "discovered!"

    def configure(self, config: dict[str, Any]) -> None:
        pass
"""
        (memory_dir / "test_plugin.py").write_text(plugin_code)

        plugins = discover_plugins(tmp_path)

        assert len(plugins) == 1
        assert plugins[0].name == "test_discovery"

    def test_discover_handles_invalid_plugin(self, tmp_path):
        memory_dir = tmp_path / ".moss" / "memory"
        memory_dir.mkdir(parents=True)

        # Create a file with syntax error
        (memory_dir / "bad.py").write_text("this is not valid python {{{")

        # Should not raise, just skip the bad file
        plugins = discover_plugins(tmp_path)
        assert plugins == []


class TestMemoryLayerDefault:
    """Tests for MemoryLayer.default() factory."""

    def test_default_creates_layer(self, tmp_path):
        layer = MemoryLayer.default(tmp_path)

        assert layer is not None
        assert layer.manager is not None
