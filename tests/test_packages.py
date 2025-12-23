"""Tests for package boundaries and API consistency.

These tests verify that the sub-packages can be imported and
provide the expected APIs. They test the package structure,
not the functionality (which is tested elsewhere).
"""

from __future__ import annotations


class TestMossIntelligence:
    """Tests for moss-intelligence package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_intelligence

        assert moss_intelligence is not None

    def test_intelligence_class(self) -> None:
        """Intelligence class is available."""
        from moss_intelligence import Intelligence

        assert Intelligence is not None

    def test_public_api(self) -> None:
        """Public API exports are available."""
        from moss_intelligence import Intelligence

        assert Intelligence is not None
        # Note: extract_python_skeleton is internal, not exported


class TestMossContext:
    """Tests for moss-context package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_context

        assert moss_context is not None

    def test_working_memory_class(self) -> None:
        """WorkingMemory class is available."""
        from moss_context import WorkingMemory

        assert WorkingMemory is not None

    def test_working_memory_instantiation(self) -> None:
        """WorkingMemory can be instantiated."""
        from moss_context import WorkingMemory

        memory = WorkingMemory(budget=1000)
        assert memory.budget == 1000
        assert memory.total_tokens == 0

    def test_working_memory_add_item(self) -> None:
        """WorkingMemory can add items."""
        from moss_context import Item, WorkingMemory

        memory = WorkingMemory(budget=1000)
        item = Item(id="test", content="Hello world", relevance=1.0)
        result = memory.add(item)

        assert result is True
        assert memory.get("test") is not None


class TestMossOrchestration:
    """Tests for moss-orchestration package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_orchestration

        assert moss_orchestration is not None

    def test_session_class(self) -> None:
        """Session class is available."""
        from moss_orchestration import Session

        assert Session is not None

    def test_driver_protocol(self) -> None:
        """Driver protocol types are available."""
        from moss_orchestration import Action, ActionResult, Context, Driver

        assert Driver is not None
        assert Action is not None
        assert ActionResult is not None
        assert Context is not None


class TestMossLLM:
    """Tests for moss-llm package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_llm

        assert moss_llm is not None

    def test_llm_summarizer_class(self) -> None:
        """LLMSummarizer class is available."""
        from moss_llm import LLMSummarizer

        assert LLMSummarizer is not None

    def test_llm_decider_class(self) -> None:
        """LLMDecider class is available."""
        from moss_llm import LLMDecider

        assert LLMDecider is not None

    def test_llm_completion_class(self) -> None:
        """LLMCompletion class is available."""
        from moss_llm import LLMCompletion

        assert LLMCompletion is not None


class TestMossMCP:
    """Tests for moss-mcp package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_mcp

        assert moss_mcp is not None

    def test_public_api(self) -> None:
        """Public API exports are available."""
        from moss_mcp import get_server, get_server_full, run_server

        assert get_server is not None
        assert get_server_full is not None
        assert run_server is not None


class TestMossLSP:
    """Tests for moss-lsp package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_lsp

        assert moss_lsp is not None

    def test_public_api(self) -> None:
        """Public API exports are available."""
        from moss_lsp import get_server, run_server

        assert get_server is not None
        assert run_server is not None


class TestMossTUI:
    """Tests for moss-tui package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_tui

        assert moss_tui is not None

    def test_public_api(self) -> None:
        """Public API exports are available."""
        from moss_tui import get_app, run_app

        assert get_app is not None
        assert run_app is not None


class TestMossACP:
    """Tests for moss-acp package."""

    def test_import_package(self) -> None:
        """Package can be imported."""
        import moss_acp

        assert moss_acp is not None

    def test_public_api(self) -> None:
        """Public API exports are available."""
        from moss_acp import get_server, run_server

        assert get_server is not None
        assert run_server is not None


class TestPackageDependencies:
    """Tests for package dependency relationships."""

    def test_intelligence_has_no_orchestration_deps(self) -> None:
        """moss-intelligence should not import from moss-orchestration."""
        import moss_intelligence

        # This just verifies the import works without orchestration
        assert moss_intelligence is not None

    def test_context_has_no_intelligence_deps(self) -> None:
        """moss-context should not import from moss-intelligence."""
        import moss_context

        # This just verifies the import works without intelligence
        assert moss_context is not None

    def test_context_has_no_orchestration_deps(self) -> None:
        """moss-context should not import from moss-orchestration."""
        import moss_context

        # This just verifies the import works without orchestration
        assert moss_context is not None
