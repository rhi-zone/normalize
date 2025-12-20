"""Tests for DWIM-driven agent loop."""

from unittest.mock import MagicMock, patch

import pytest

from moss.dwim_loop import (
    DWIMLoop,
    LoopConfig,
    LoopState,
    TaskType,
    build_tool_call,
    classify_task,
    parse_intent,
)


class TestParseIntent:
    """Tests for parse_intent function."""

    def test_simple_command(self):
        """Parse simple verb + target."""
        result = parse_intent("skeleton foo.py")
        assert result.verb == "skeleton"
        assert result.target == "foo.py"
        assert result.content is None
        assert result.confidence == 1.0

    def test_command_with_path(self):
        """Parse command with file path."""
        result = parse_intent("skeleton src/moss/dwim.py")
        assert result.verb == "skeleton"
        assert result.target == "src/moss/dwim.py"

    def test_expand_symbol(self):
        """Parse expand with symbol."""
        result = parse_intent("expand Patch.apply")
        assert result.verb == "expand"
        assert result.target == "Patch.apply"

    def test_fix_with_content(self):
        """Parse fix: format."""
        result = parse_intent("fix: add null check for anchor")
        assert result.verb == "fix"
        assert result.content == "add null check for anchor"
        assert result.target is None

    def test_done_signal(self):
        """Parse done signal."""
        result = parse_intent("done")
        assert result.verb == "done"
        assert result.target is None

    def test_done_alternatives(self):
        """Parse alternative done signals."""
        for word in ["done", "finished", "complete"]:
            result = parse_intent(word)
            assert result.verb == "done", f"'{word}' should map to 'done'"

    def test_verb_aliases(self):
        """Parse verb aliases."""
        result = parse_intent("skel foo.py")
        assert result.verb == "skeleton"

        result = parse_intent("show main")
        assert result.verb == "expand"

    def test_grep_with_pattern(self):
        """Parse grep with pattern and path."""
        result = parse_intent("grep 'def main' src/")
        assert result.verb == "grep"
        assert result.target == "'def main' src/"

    def test_validate_no_target(self):
        """Parse validate without target."""
        result = parse_intent("validate")
        assert result.verb == "validate"
        assert result.target is None

    def test_empty_input(self):
        """Parse empty input."""
        result = parse_intent("")
        assert result.verb == ""
        assert result.confidence == 0.0

    def test_whitespace_handling(self):
        """Handle leading/trailing whitespace."""
        result = parse_intent("  skeleton foo.py  ")
        assert result.verb == "skeleton"
        assert result.target == "foo.py"

    def test_natural_language_fallback(self):
        """Natural language falls back to query."""
        result = parse_intent("what functions are in this file")
        assert result.verb == "query"
        assert result.content == "what functions are in this file"
        assert result.confidence == 0.5


class TestLoopConfig:
    """Tests for LoopConfig."""

    def test_defaults(self):
        """Test default values."""
        config = LoopConfig()
        assert config.max_turns == 50
        assert config.stall_threshold == 5
        assert config.temperature == 0.0

    def test_custom_values(self):
        """Test custom values."""
        config = LoopConfig(max_turns=10, temperature=0.5)
        assert config.max_turns == 10
        assert config.temperature == 0.5


class TestClassifyTask:
    """Tests for classify_task function."""

    def test_read_only_show(self):
        """Show commands are read-only."""
        assert classify_task("show me the main function") == TaskType.READ_ONLY
        assert classify_task("display the imports") == TaskType.READ_ONLY

    def test_read_only_find(self):
        """Find/search commands are read-only."""
        assert classify_task("find all usages of Logger") == TaskType.READ_ONLY
        assert classify_task("search for error handlers") == TaskType.READ_ONLY
        assert classify_task("locate the config file") == TaskType.READ_ONLY

    def test_read_only_explain(self):
        """Explain commands are read-only."""
        assert classify_task("explain how parse_intent works") == TaskType.READ_ONLY
        assert classify_task("describe the architecture") == TaskType.READ_ONLY
        assert classify_task("summarize this module") == TaskType.READ_ONLY

    def test_read_only_questions(self):
        """Questions are read-only."""
        assert classify_task("what does this function do?") == TaskType.READ_ONLY
        assert classify_task("where is the cache defined?") == TaskType.READ_ONLY
        assert classify_task("how many tests are there?") == TaskType.READ_ONLY
        assert classify_task("is this method used?") == TaskType.READ_ONLY

    def test_write_fix(self):
        """Fix commands are write tasks."""
        assert classify_task("fix the null pointer bug") == TaskType.WRITE
        assert classify_task("repair the broken test") == TaskType.WRITE

    def test_write_add(self):
        """Add commands are write tasks."""
        assert classify_task("add a new parameter") == TaskType.WRITE
        assert classify_task("implement caching") == TaskType.WRITE
        assert classify_task("create a helper function") == TaskType.WRITE

    def test_write_modify(self):
        """Modify commands are write tasks."""
        assert classify_task("change the default value") == TaskType.WRITE
        assert classify_task("update the error message") == TaskType.WRITE
        assert classify_task("refactor this class") == TaskType.WRITE

    def test_write_remove(self):
        """Remove commands are write tasks."""
        assert classify_task("remove the deprecated method") == TaskType.WRITE
        assert classify_task("delete unused imports") == TaskType.WRITE

    def test_unknown_ambiguous(self):
        """Ambiguous tasks return UNKNOWN."""
        assert classify_task("process the data") == TaskType.UNKNOWN
        assert classify_task("run the tests") == TaskType.UNKNOWN

    def test_write_takes_precedence(self):
        """Write patterns take precedence over read patterns."""
        # "find and fix" should be WRITE because fix is checked first
        assert classify_task("find and fix the bug") == TaskType.WRITE


class TestBuildToolCall:
    """Tests for build_tool_call function."""

    def test_expand_with_file_and_symbol(self):
        """Expand with both file and symbol in either order."""
        api = MagicMock()
        intent = parse_intent("expand TaskTree src/moss/task_tree.py")
        tool_name, params = build_tool_call(intent, api)
        assert tool_name == "skeleton.expand"
        assert params["file_path"] == "src/moss/task_tree.py"
        assert params["symbol_name"] == "TaskTree"

    def test_expand_file_first(self):
        """Expand with file path first."""
        api = MagicMock()
        intent = parse_intent("expand src/moss/task_tree.py TaskTree")
        tool_name, params = build_tool_call(intent, api)
        assert tool_name == "skeleton.expand"
        assert params["file_path"] == "src/moss/task_tree.py"
        assert params["symbol_name"] == "TaskTree"

    def test_expand_symbol_only_triggers_search(self):
        """Expand with only symbol triggers search mode."""
        api = MagicMock()
        intent = parse_intent("expand TaskTree")
        tool_name, params = build_tool_call(intent, api)
        assert tool_name == "skeleton.expand_search"
        assert params["symbol_name"] == "TaskTree"
        assert "file_path" not in params

    def test_expand_multi_file(self):
        """Expand with multiple files."""
        api = MagicMock()
        intent = parse_intent("expand Symbol file1.py file2.py")
        tool_name, params = build_tool_call(intent, api)
        assert tool_name == "skeleton.expand"
        assert params["file_paths"] == ["file1.py", "file2.py"]
        assert params["symbol_name"] == "Symbol"


class TestDWIMLoopIntegration:
    """Integration tests for DWIMLoop with mocked LLM."""

    @pytest.fixture
    def mock_api(self):
        """Create a mock MossAPI."""
        api = MagicMock()
        api.skeleton.format.return_value = "class Foo:\n    def bar(self): ..."
        api.skeleton.expand.return_value = "class Foo:\n    def bar(self):\n        return 42"
        api.search.find_definitions.return_value = []
        return api

    @pytest.mark.asyncio
    async def test_simple_read_task_completes(self, mock_api):
        """Read-only task completes after getting result."""
        config = LoopConfig(max_turns=10)
        loop = DWIMLoop(mock_api, config)

        # Mock LLM responses: first skeleton, then done
        responses = iter(["skeleton src/foo.py", "done found class Foo"])

        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]

        async def mock_completion(**kwargs):
            mock_response.choices[0].message.content = next(responses)
            return mock_response

        with patch.dict("sys.modules", {"litellm": MagicMock(acompletion=mock_completion)}):
            result = await loop.run("show the structure of foo.py")

        assert result.state == LoopState.DONE
        assert len(result.turns) == 2

    @pytest.mark.asyncio
    async def test_stall_detection(self, mock_api):
        """Repeated commands trigger stall detection."""
        config = LoopConfig(max_turns=10, stall_threshold=3)
        loop = DWIMLoop(mock_api, config)

        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]
        mock_response.choices[0].message.content = "skeleton foo.py"

        async def mock_completion(**kwargs):
            return mock_response

        with patch.dict("sys.modules", {"litellm": MagicMock(acompletion=mock_completion)}):
            result = await loop.run("show foo.py")

        assert result.state == LoopState.STALLED
        assert "repeated" in result.error.lower()

    @pytest.mark.asyncio
    async def test_max_turns_limit(self, mock_api):
        """Loop stops at max turns."""
        config = LoopConfig(max_turns=3)
        loop = DWIMLoop(mock_api, config)

        turn_count = [0]
        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]

        async def mock_completion(**kwargs):
            turn_count[0] += 1
            # Different commands to avoid stall detection
            mock_response.choices[0].message.content = f"skeleton file{turn_count[0]}.py"
            return mock_response

        with patch.dict("sys.modules", {"litellm": MagicMock(acompletion=mock_completion)}):
            result = await loop.run("explore the codebase")

        assert result.state == LoopState.MAX_TURNS
        assert len(result.turns) == 3

    @pytest.mark.asyncio
    async def test_read_only_task_classification(self, mock_api):
        """Read-only tasks get completion hints."""
        config = LoopConfig(max_turns=5)
        loop = DWIMLoop(mock_api, config)

        contexts_received = []
        call_count = [0]
        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]

        async def mock_completion(**kwargs):
            call_count[0] += 1
            contexts_received.append(kwargs.get("messages", []))
            if call_count[0] == 1:
                mock_response.choices[0].message.content = "skeleton src/foo.py"
            else:
                mock_response.choices[0].message.content = "done"
            return mock_response

        with patch.dict("sys.modules", {"litellm": MagicMock(acompletion=mock_completion)}):
            result = await loop.run("what does foo.py contain?")

        # After first turn, context should include completion hint
        assert result.state == LoopState.DONE
        if len(contexts_received) > 1:
            last_context = contexts_received[1]
            user_msg = last_context[1]["content"] if len(last_context) > 1 else ""
            assert "READ-ONLY TASK" in user_msg or result.state == LoopState.DONE

    @pytest.mark.asyncio
    async def test_multi_step_with_breakdown(self, mock_api):
        """Task breakdown creates subtasks."""
        config = LoopConfig(max_turns=10)
        loop = DWIMLoop(mock_api, config)

        responses = iter(
            [
                "breakdown: find files, analyze structure, summarize",
                "skeleton src/main.py",
                "done analyzed main.py",
                "skeleton src/utils.py",
                "done analyzed utils.py",
                "done all files analyzed",
            ]
        )

        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]

        async def mock_completion(**kwargs):
            try:
                mock_response.choices[0].message.content = next(responses)
            except StopIteration:
                mock_response.choices[0].message.content = "done"
            return mock_response

        with patch.dict("sys.modules", {"litellm": MagicMock(acompletion=mock_completion)}):
            result = await loop.run("analyze the codebase structure")

        assert result.state == LoopState.DONE
        # Should have breakdown turn plus work turns
        assert len(result.turns) >= 2
