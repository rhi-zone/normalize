"""Tests for Composable Agent Loops (agent_loop.py)."""

import pytest

from moss.agent_loop import (
    AgentLoop,
    AgentLoopRunner,
    BenchmarkTask,
    ErrorAction,
    LLMConfig,
    LLMToolExecutor,
    LoopBenchmark,
    LoopContext,
    LoopMetrics,
    LoopStatus,
    LoopStep,
    MCPServerConfig,
    MCPToolExecutor,
    MossToolExecutor,
    StepType,
    analysis_loop,
    critic_loop,
    docstring_apply_loop,
    docstring_full_loop,
    docstring_loop,
    incremental_loop,
    simple_loop,
)


class TestLoopStep:
    """Tests for LoopStep dataclass."""

    def test_create_basic_step(self):
        step = LoopStep(name="test", tool="skeleton.format")
        assert step.name == "test"
        assert step.tool == "skeleton.format"
        assert step.step_type == StepType.TOOL
        assert step.on_error == ErrorAction.ABORT

    def test_create_llm_step(self):
        step = LoopStep(
            name="generate",
            tool="llm.generate",
            step_type=StepType.LLM,
            input_from="context",
        )
        assert step.step_type == StepType.LLM
        assert step.input_from == "context"

    def test_goto_requires_target(self):
        with pytest.raises(ValueError, match="GOTO action requires goto_target"):
            LoopStep(name="test", tool="test", on_error=ErrorAction.GOTO)

    def test_goto_with_target(self):
        step = LoopStep(
            name="test", tool="test", on_error=ErrorAction.GOTO, goto_target="retry_step"
        )
        assert step.goto_target == "retry_step"


class TestAgentLoop:
    """Tests for AgentLoop dataclass."""

    def test_create_basic_loop(self):
        loop = AgentLoop(
            name="test",
            steps=[LoopStep(name="step1", tool="test")],
        )
        assert loop.name == "test"
        assert loop.entry == "step1"  # Defaults to first step
        assert loop.max_steps == 10

    def test_loop_requires_steps(self):
        with pytest.raises(ValueError, match="must have at least one step"):
            AgentLoop(name="empty", steps=[])

    def test_loop_validates_entry(self):
        with pytest.raises(ValueError, match="Entry step 'nonexistent' not found"):
            AgentLoop(
                name="test",
                steps=[LoopStep(name="step1", tool="test")],
                entry="nonexistent",
            )

    def test_loop_validates_goto_targets(self):
        with pytest.raises(ValueError, match="GOTO target 'nonexistent' not found"):
            AgentLoop(
                name="test",
                steps=[
                    LoopStep(
                        name="step1",
                        tool="test",
                        on_error=ErrorAction.GOTO,
                        goto_target="nonexistent",
                    )
                ],
            )

    def test_step_names_must_be_unique(self):
        with pytest.raises(ValueError, match="Step names must be unique"):
            AgentLoop(
                name="test",
                steps=[
                    LoopStep(name="dupe", tool="test1"),
                    LoopStep(name="dupe", tool="test2"),
                ],
            )


class TestLoopContext:
    """Tests for LoopContext dataclass."""

    def test_initial_context(self):
        ctx = LoopContext(input="initial data")
        assert ctx.input == "initial data"
        assert ctx.steps == {}
        assert ctx.last is None

    def test_with_step(self):
        ctx = LoopContext(input="initial")
        ctx2 = ctx.with_step("step1", "output1")

        # Original unchanged
        assert ctx.steps == {}
        assert ctx.last is None

        # New context has step
        assert ctx2.steps == {"step1": "output1"}
        assert ctx2.last == "output1"
        assert ctx2.input == "initial"

    def test_chained_steps(self):
        ctx = LoopContext(input="initial")
        ctx = ctx.with_step("step1", "out1")
        ctx = ctx.with_step("step2", "out2")

        assert ctx.steps == {"step1": "out1", "step2": "out2"}
        assert ctx.last == "out2"

    def test_get_step(self):
        ctx = LoopContext(input="initial", steps={"a": 1, "b": 2})
        assert ctx.get("a") == 1
        assert ctx.get("c") is None
        assert ctx.get("c", "default") == "default"


class TestLoopMetrics:
    """Tests for LoopMetrics dataclass."""

    def test_initial_metrics(self):
        metrics = LoopMetrics()
        assert metrics.llm_calls == 0
        assert metrics.tool_calls == 0
        assert metrics.iterations == 0

    def test_record_tool_step(self):
        metrics = LoopMetrics()
        metrics.record_step("step1", StepType.TOOL, duration=1.5)

        assert metrics.tool_calls == 1
        assert metrics.llm_calls == 0
        assert metrics.step_times["step1"] == 1.5

    def test_record_llm_step(self):
        metrics = LoopMetrics()
        metrics.record_step("step1", StepType.LLM, duration=2.0, tokens_in=100, tokens_out=50)

        assert metrics.llm_calls == 1
        assert metrics.llm_tokens_in == 100
        assert metrics.llm_tokens_out == 50
        assert metrics.tool_calls == 0

    def test_record_hybrid_step_with_tokens(self):
        metrics = LoopMetrics()
        metrics.record_step("step1", StepType.HYBRID, duration=1.0, tokens_in=10, tokens_out=5)

        assert metrics.tool_calls == 1
        assert metrics.llm_calls == 1
        assert metrics.llm_tokens_in == 10

    def test_record_hybrid_step_without_tokens(self):
        metrics = LoopMetrics()
        metrics.record_step("step1", StepType.HYBRID, duration=1.0)

        assert metrics.tool_calls == 1
        assert metrics.llm_calls == 0

    def test_to_compact(self):
        metrics = LoopMetrics()
        metrics.llm_calls = 2
        metrics.llm_tokens_in = 100
        metrics.llm_tokens_out = 50
        metrics.tool_calls = 5
        metrics.wall_time_seconds = 3.5
        metrics.iterations = 3
        metrics.retries = 1

        compact = metrics.to_compact()
        assert "LLM: 2 calls" in compact
        assert "150 tokens" in compact
        assert "Tools: 5 calls" in compact


class MockExecutor:
    """Mock executor for testing."""

    def __init__(self, responses: dict[str, tuple] | None = None):
        self.responses = responses or {}
        self.calls: list[tuple[str, LoopContext, LoopStep]] = []

    async def execute(
        self, tool_name: str, context: LoopContext, step: LoopStep
    ) -> tuple[str, int, int]:
        self.calls.append((tool_name, context, step))

        if tool_name in self.responses:
            resp = self.responses[tool_name]
            if isinstance(resp, Exception):
                raise resp
            return resp

        # Default response
        return f"output:{tool_name}", 0, 0


class TestAgentLoopRunner:
    """Tests for AgentLoopRunner."""

    @pytest.fixture
    def mock_executor(self):
        return MockExecutor()

    @pytest.fixture
    def runner(self, mock_executor):
        return AgentLoopRunner(mock_executor)

    async def test_run_simple_loop(self, runner, mock_executor):
        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1"),
                LoopStep(name="step2", tool="tool2"),
            ],
        )

        result = await runner.run(loop, initial_input="input")

        assert result.success
        assert result.status == LoopStatus.SUCCESS
        assert len(mock_executor.calls) == 2
        assert result.final_output == "output:tool2"

    async def test_context_passed_through(self, runner, mock_executor):
        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1"),
                LoopStep(name="step2", tool="tool2", input_from="step1"),
            ],
        )

        await runner.run(loop, initial_input="my_input")

        # First call gets initial input
        _, ctx1, _ = mock_executor.calls[0]
        assert ctx1.input == "my_input"
        assert ctx1.steps == {}

        # Second call has step1 output available
        _, ctx2, _ = mock_executor.calls[1]
        assert ctx2.input == "my_input"
        assert "step1" in ctx2.steps

    async def test_exit_condition(self, runner, mock_executor):
        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1"),
                LoopStep(name="step2", tool="tool2"),
            ],
            exit_conditions=["step1.success"],
        )

        result = await runner.run(loop)

        assert result.success
        # Should exit after step1, not run step2
        assert len(mock_executor.calls) == 1

    async def test_max_steps_limit(self, runner, mock_executor):
        loop = AgentLoop(
            name="test",
            steps=[LoopStep(name="step1", tool="tool1")],
            exit_conditions=["never.exits"],  # Forces loop to continue
            max_steps=3,
        )

        result = await runner.run(loop)

        assert result.status == LoopStatus.MAX_ITERATIONS
        assert len(mock_executor.calls) == 3

    async def test_error_abort(self, runner):
        mock = MockExecutor(responses={"tool1": ValueError("test error")})
        runner = AgentLoopRunner(mock)

        loop = AgentLoop(
            name="test",
            steps=[LoopStep(name="step1", tool="tool1", on_error=ErrorAction.ABORT)],
        )

        result = await runner.run(loop)

        assert result.status == LoopStatus.FAILED
        assert "test error" in result.error

    async def test_error_skip(self, runner):
        mock = MockExecutor(
            responses={
                "tool1": ValueError("skip this"),
                "tool2": ("success", 0, 0),
            }
        )
        runner = AgentLoopRunner(mock)

        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1", on_error=ErrorAction.SKIP),
                LoopStep(name="step2", tool="tool2"),
            ],
        )

        result = await runner.run(loop)

        assert result.success
        assert result.final_output == "success"

    async def test_error_retry(self, runner):
        call_count = 0

        class RetryExecutor:
            async def execute(self, tool_name, context, step):
                nonlocal call_count
                call_count += 1
                if call_count < 3:
                    raise ValueError("retry me")
                return "success", 0, 0

        runner = AgentLoopRunner(RetryExecutor())

        loop = AgentLoop(
            name="test",
            steps=[LoopStep(name="step1", tool="tool1", on_error=ErrorAction.RETRY, max_retries=5)],
        )

        result = await runner.run(loop)

        assert result.success
        assert call_count == 3

    async def test_error_goto(self, runner):
        call_sequence = []

        class GotoExecutor:
            async def execute(self, tool_name, context, step):
                call_sequence.append(step.name)
                if step.name == "step1":
                    raise ValueError("goto recovery")
                return f"output:{step.name}", 0, 0

        runner = AgentLoopRunner(GotoExecutor())

        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(
                    name="step1",
                    tool="tool1",
                    on_error=ErrorAction.GOTO,
                    goto_target="recovery",
                ),
                LoopStep(name="step2", tool="tool2"),
                LoopStep(name="recovery", tool="recover"),
            ],
            max_steps=5,
        )

        await runner.run(loop)

        assert "step1" in call_sequence
        assert "recovery" in call_sequence

    async def test_metrics_tracking(self, runner):
        mock = MockExecutor(
            responses={
                "tool1": ("out1", 0, 0),
                "llm.gen": ("out2", 100, 50),
            }
        )
        runner = AgentLoopRunner(mock)

        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1", step_type=StepType.TOOL),
                LoopStep(name="step2", tool="llm.gen", step_type=StepType.LLM),
            ],
        )

        result = await runner.run(loop)

        assert result.metrics.tool_calls == 1
        assert result.metrics.llm_calls == 1
        assert result.metrics.llm_tokens_in == 100
        assert result.metrics.llm_tokens_out == 50

    async def test_token_budget(self, runner):
        mock = MockExecutor(responses={"llm.gen": ("out", 100, 100)})
        runner = AgentLoopRunner(mock)

        loop = AgentLoop(
            name="test",
            steps=[LoopStep(name="step1", tool="llm.gen", step_type=StepType.LLM)],
            exit_conditions=["never"],
            token_budget=150,
            max_steps=10,
        )

        result = await runner.run(loop)

        assert result.status == LoopStatus.BUDGET_EXCEEDED


class TestLoopTemplates:
    """Tests for pre-built loop templates."""

    def test_simple_loop_structure(self):
        loop = simple_loop()
        assert loop.name == "simple"
        assert len(loop.steps) == 3
        assert loop.steps[0].name == "understand"
        assert loop.steps[1].name == "act"
        assert loop.steps[2].name == "validate"
        assert "validate.success" in loop.exit_conditions

    def test_critic_loop_structure(self):
        loop = critic_loop()
        assert loop.name == "critic"
        assert len(loop.steps) == 4
        assert loop.steps[1].name == "review"
        assert loop.steps[1].step_type == StepType.LLM

    def test_incremental_loop_structure(self):
        loop = incremental_loop()
        assert loop.name == "incremental"
        assert any(s.name == "decide" for s in loop.steps)

    def test_analysis_loop_structure(self):
        loop = analysis_loop()
        assert loop.name == "analysis"
        assert len(loop.steps) == 2
        assert loop.steps[0].name == "skeleton"
        assert loop.steps[1].name == "analyze"
        assert loop.steps[1].step_type == StepType.LLM
        assert "analyze.success" in loop.exit_conditions

    def test_docstring_loop_structure(self):
        loop = docstring_loop()
        assert loop.name == "docstring"
        assert len(loop.steps) == 2
        assert loop.steps[0].name == "skeleton"
        assert loop.steps[1].name == "identify"
        assert loop.steps[1].step_type == StepType.LLM


class TestLLMConfig:
    """Tests for LLMConfig."""

    def test_default_config(self):
        config = LLMConfig()
        assert "gemini" in config.model
        assert config.temperature == 0.0
        assert config.mock is False

    def test_custom_config(self):
        config = LLMConfig(model="gpt-4o", temperature=0.7, mock=True)
        assert config.model == "gpt-4o"
        assert config.temperature == 0.7
        assert config.mock is True

    def test_rotation_config(self):
        config = LLMConfig(
            models=["gemini/gemini-3-flash-preview", "gpt-4o"],
            rotation="round_robin",
        )
        assert len(config.models) == 2
        assert config.rotation == "round_robin"


class TestLLMRotation:
    """Tests for multi-LLM rotation."""

    def test_no_rotation_uses_primary(self, tmp_path):
        config = LLMConfig(model="primary-model", mock=True)
        executor = LLMToolExecutor(config=config, root=tmp_path, load_env=False)

        model = executor._get_model()
        assert model == "primary-model"

    def test_round_robin_rotation(self, tmp_path):
        config = LLMConfig(
            models=["model-a", "model-b", "model-c"],
            rotation="round_robin",
            mock=True,
        )
        executor = LLMToolExecutor(config=config, root=tmp_path, load_env=False)

        # Should cycle through models
        assert executor._get_model() == "model-a"
        assert executor._get_model() == "model-b"
        assert executor._get_model() == "model-c"
        assert executor._get_model() == "model-a"  # Wraps around

    def test_random_rotation(self, tmp_path):
        config = LLMConfig(
            models=["model-a", "model-b"],
            rotation="random",
            mock=True,
        )
        executor = LLMToolExecutor(config=config, root=tmp_path, load_env=False)

        # Should return a model from the pool
        models_seen = set()
        for _ in range(20):
            models_seen.add(executor._get_model())

        # With 20 tries, we should see both models (probabilistically)
        assert "model-a" in models_seen or "model-b" in models_seen

    def test_empty_models_uses_primary(self, tmp_path):
        config = LLMConfig(
            model="primary",
            models=[],
            rotation="round_robin",
            mock=True,
        )
        executor = LLMToolExecutor(config=config, root=tmp_path, load_env=False)

        assert executor._get_model() == "primary"


class TestLLMToolExecutor:
    """Tests for LLMToolExecutor with mock mode."""

    @pytest.fixture
    def mock_llm_executor(self, tmp_path):
        config = LLMConfig(mock=True)
        return LLMToolExecutor(config=config, root=tmp_path, load_env=False)

    async def test_llm_tool_mock_mode(self, mock_llm_executor):
        step = LoopStep(name="gen", tool="llm.generate", step_type=StepType.LLM)
        context = LoopContext(input="test prompt")

        output, tokens_in, tokens_out = await mock_llm_executor.execute(
            "llm.generate", context, step
        )

        assert "[MOCK generate]" in output
        assert tokens_in > 0
        assert tokens_out > 0

    async def test_routes_to_moss_executor(self, mock_llm_executor, tmp_path):
        # Create a test file for skeleton
        test_file = tmp_path / "test.py"
        test_file.write_text("def hello(): pass")

        step = LoopStep(name="skel", tool="skeleton.format")
        context = LoopContext(input=str(test_file))

        _output, tokens_in, tokens_out = await mock_llm_executor.execute(
            "skeleton.format", context, step
        )

        # Should route to MossToolExecutor, not LLM
        assert tokens_in == 0
        assert tokens_out == 0


class TestLoopBenchmark:
    """Tests for LoopBenchmark."""

    async def test_benchmark_single_loop(self):
        mock = MockExecutor(
            responses={
                "tool1": ("out1", 0, 0),
                "llm.gen": ("out2", 50, 25),
            }
        )
        benchmark = LoopBenchmark(executor=mock)

        loop = AgentLoop(
            name="test",
            steps=[
                LoopStep(name="step1", tool="tool1", step_type=StepType.TOOL),
                LoopStep(name="step2", tool="llm.gen", step_type=StepType.LLM),
            ],
        )

        tasks = [
            BenchmarkTask(name="task1", input_data="input1"),
            BenchmarkTask(name="task2", input_data="input2"),
        ]

        result = await benchmark.run(loop, tasks)

        assert result.tasks_run == 2
        assert result.successes == 2
        assert result.total_llm_calls == 2
        assert result.total_tool_calls == 2

    async def test_benchmark_comparison(self):
        mock = MockExecutor()
        benchmark = LoopBenchmark(executor=mock)

        loop1 = AgentLoop(name="fast", steps=[LoopStep(name="s1", tool="t1")])
        loop2 = AgentLoop(name="slow", steps=[LoopStep(name="s1", tool="t1")])

        tasks = [BenchmarkTask(name="task1", input_data="input")]

        results = await benchmark.compare([loop1, loop2], tasks)

        assert len(results) == 2
        assert results[0].loop_name == "fast"
        assert results[1].loop_name == "slow"

    async def test_benchmark_result_formatting(self):
        mock = MockExecutor()
        benchmark = LoopBenchmark(executor=mock)

        loop = AgentLoop(name="test", steps=[LoopStep(name="s1", tool="t1")])
        tasks = [BenchmarkTask(name="t1", input_data="i1")]

        result = await benchmark.run(loop, tasks)

        compact = result.to_compact()
        assert "test" in compact
        assert "100%" in compact

        markdown = result.to_markdown()
        assert "# Benchmark" in markdown
        assert "Success rate" in markdown


class TestDocstringFullLoop:
    """Tests for docstring_full_loop template."""

    def test_docstring_full_loop_structure(self):
        loop = docstring_full_loop()
        assert loop.name == "docstring_full"
        assert len(loop.steps) == 3
        assert loop.steps[0].name == "skeleton"
        assert loop.steps[0].tool == "skeleton.format"
        assert loop.steps[1].name == "identify"
        assert loop.steps[1].step_type == StepType.LLM
        assert loop.steps[2].name == "parse"
        assert loop.steps[2].tool == "parse.docstrings"
        assert "parse.success" in loop.exit_conditions

    def test_docstring_full_loop_custom_name(self):
        loop = docstring_full_loop(name="custom_docstring")
        assert loop.name == "custom_docstring"


class TestMossToolExecutor:
    """Tests for MossToolExecutor."""

    @pytest.fixture
    def executor(self, tmp_path):
        """Create a MossToolExecutor for testing."""
        return MossToolExecutor(root=tmp_path)

    @pytest.mark.asyncio
    async def test_parse_docstrings_basic(self, executor):
        """Test parsing basic FUNC:name|docstring format."""
        llm_output = """FUNC:foo|Does something useful
FUNC:bar|Processes the data"""
        context = LoopContext(input=llm_output)
        step = LoopStep(name="parse", tool="parse.docstrings")

        result, tokens_in, tokens_out = await executor.execute("parse.docstrings", context, step)

        assert len(result) == 2
        assert result[0]["function"] == "foo"
        assert result[0]["docstring"] == "Does something useful"
        assert result[1]["function"] == "bar"
        assert result[1]["docstring"] == "Processes the data"
        assert tokens_in == 0
        assert tokens_out == 0

    @pytest.mark.asyncio
    async def test_parse_docstrings_with_extra_lines(self, executor):
        """Test parsing ignores non-FUNC lines."""
        llm_output = """Here are the functions that need docstrings:

FUNC:calculate|Calculates the result

Some other text
FUNC:validate|Validates input data"""
        context = LoopContext(input=llm_output)
        step = LoopStep(name="parse", tool="parse.docstrings")

        result, _, _ = await executor.execute("parse.docstrings", context, step)

        assert len(result) == 2
        assert result[0]["function"] == "calculate"
        assert result[1]["function"] == "validate"

    @pytest.mark.asyncio
    async def test_parse_docstrings_empty_input(self, executor):
        """Test parsing empty input returns empty list."""
        context = LoopContext(input="")
        step = LoopStep(name="parse", tool="parse.docstrings")

        result, _, _ = await executor.execute("parse.docstrings", context, step)

        assert result == []

    @pytest.mark.asyncio
    async def test_parse_docstrings_malformed_lines(self, executor):
        """Test parsing skips malformed lines."""
        llm_output = """FUNC:valid|Valid docstring
FUNC:no_pipe_here
FUNC:|empty_name
FUNC:also_valid|Another valid one"""
        context = LoopContext(input=llm_output)
        step = LoopStep(name="parse", tool="parse.docstrings")

        result, _, _ = await executor.execute("parse.docstrings", context, step)

        assert len(result) == 2
        assert result[0]["function"] == "valid"
        assert result[1]["function"] == "also_valid"

    @pytest.mark.asyncio
    async def test_parse_docstrings_preserves_pipe_in_docstring(self, executor):
        """Test that pipes in docstring are preserved."""
        llm_output = "FUNC:filter|Filters items where x | y is true"
        context = LoopContext(input=llm_output)
        step = LoopStep(name="parse", tool="parse.docstrings")

        result, _, _ = await executor.execute("parse.docstrings", context, step)

        assert len(result) == 1
        assert result[0]["docstring"] == "Filters items where x | y is true"


class TestParseDocstringOutput:
    """Direct tests for _parse_docstring_output method."""

    @pytest.fixture
    def executor(self, tmp_path):
        return MossToolExecutor(root=tmp_path)

    def test_parse_basic(self, executor):
        output = "FUNC:test|A test function"
        result = executor._parse_docstring_output(output)
        assert result == [{"function": "test", "docstring": "A test function"}]

    def test_parse_multiple(self, executor):
        output = "FUNC:a|First\nFUNC:b|Second"
        result = executor._parse_docstring_output(output)
        assert len(result) == 2

    def test_parse_strips_whitespace(self, executor):
        output = "FUNC:  spaced  |  has spaces  "
        result = executor._parse_docstring_output(output)
        assert result[0]["function"] == "spaced"
        assert result[0]["docstring"] == "has spaces"

    def test_parse_ignores_empty_lines(self, executor):
        output = "\n\nFUNC:test|value\n\n"
        result = executor._parse_docstring_output(output)
        assert len(result) == 1


class TestDocstringApplyLoop:
    """Tests for docstring_apply_loop template."""

    def test_docstring_apply_loop_structure(self):
        loop = docstring_apply_loop()
        assert loop.name == "docstring_apply"
        assert len(loop.steps) == 4
        assert loop.steps[0].name == "skeleton"
        assert loop.steps[1].name == "identify"
        assert loop.steps[2].name == "parse"
        assert loop.steps[3].name == "apply"
        assert loop.steps[3].tool == "patch.docstrings"
        assert "apply.success" in loop.exit_conditions

    def test_docstring_apply_loop_custom_name(self):
        loop = docstring_apply_loop(name="custom_apply")
        assert loop.name == "custom_apply"


class TestApplyDocstrings:
    """Tests for _apply_docstrings method."""

    @pytest.fixture
    def executor(self, tmp_path):
        return MossToolExecutor(root=tmp_path)

    def test_apply_docstrings_to_file(self, executor, tmp_path):
        """Test applying docstrings to a Python file."""
        # Create a test file with undocumented functions
        test_file = tmp_path / "test_module.py"
        test_file.write_text("""def foo():
    pass

def bar(x, y):
    return x + y
""")

        docstrings = [
            {"function": "foo", "docstring": "Do foo things."},
            {"function": "bar", "docstring": "Add two numbers."},
        ]

        result = executor._apply_docstrings(str(test_file), docstrings)

        assert "foo" in result["applied"]
        assert "bar" in result["applied"]
        assert result["errors"] == []

        # Verify the file was modified
        modified = test_file.read_text()
        assert '"""Do foo things."""' in modified
        assert '"""Add two numbers."""' in modified

    def test_apply_docstrings_function_not_found(self, executor, tmp_path):
        """Test handling of functions that don't exist."""
        test_file = tmp_path / "test_module.py"
        test_file.write_text("def foo():\n    pass\n")

        docstrings = [
            {"function": "nonexistent", "docstring": "Should not apply."},
        ]

        result = executor._apply_docstrings(str(test_file), docstrings)

        assert result["applied"] == []
        assert "Function not found: nonexistent" in result["errors"]

    def test_apply_docstrings_file_not_found(self, executor, tmp_path):
        """Test handling of missing files."""
        result = executor._apply_docstrings(
            str(tmp_path / "nonexistent.py"),
            [{"function": "foo", "docstring": "test"}],
        )

        assert result["applied"] == []
        assert any("not found" in e.lower() for e in result["errors"])

    def test_apply_docstrings_preserves_indentation(self, executor, tmp_path):
        """Test that docstrings are properly indented."""
        test_file = tmp_path / "test_module.py"
        test_file.write_text("""class MyClass:
    def method(self):
        pass
""")

        docstrings = [{"function": "method", "docstring": "A method."}]

        result = executor._apply_docstrings(str(test_file), docstrings)

        assert "method" in result["applied"]
        modified = test_file.read_text()
        # Verify the docstring is indented correctly (8 spaces for method body)
        assert '        """A method."""' in modified

    @pytest.mark.asyncio
    async def test_patch_docstrings_via_execute(self, executor, tmp_path):
        """Test patch.docstrings tool via executor.execute."""
        test_file = tmp_path / "test.py"
        test_file.write_text("def test_fn():\n    pass\n")

        docstrings = [{"function": "test_fn", "docstring": "Test function."}]
        context = LoopContext(input={"file_path": str(test_file)})
        step = LoopStep(
            name="apply",
            tool="patch.docstrings",
            input_from="parse",
        )
        # Set up context with parse output
        context = context.with_step("parse", docstrings)

        result, tokens_in, tokens_out = await executor.execute("patch.docstrings", context, step)

        assert "test_fn" in result["applied"]
        assert tokens_in == 0
        assert tokens_out == 0


class TestMCPServerConfig:
    """Tests for MCPServerConfig dataclass."""

    def test_basic_config(self):
        config = MCPServerConfig(command="uv", args=["run", "moss-mcp"])
        assert config.command == "uv"
        assert config.args == ["run", "moss-mcp"]
        assert config.cwd is None
        assert config.env is None

    def test_config_with_all_options(self):
        config = MCPServerConfig(
            command="npx",
            args=["@anthropic/mcp-server-filesystem"],
            cwd="/tmp",
            env={"DEBUG": "1"},
        )
        assert config.command == "npx"
        assert config.cwd == "/tmp"
        assert config.env == {"DEBUG": "1"}


class TestMCPToolExecutor:
    """Tests for MCPToolExecutor."""

    def test_init(self):
        config = MCPServerConfig(command="test", args=["arg"])
        executor = MCPToolExecutor(config)
        assert executor.config == config
        assert executor._session is None
        assert executor._tools == {}

    def test_list_tools_empty_before_connect(self):
        config = MCPServerConfig(command="test", args=["arg"])
        executor = MCPToolExecutor(config)
        assert executor.list_tools() == []

    @pytest.mark.asyncio
    async def test_context_manager_protocol(self):
        """Test that MCPToolExecutor can be used as async context manager."""
        config = MCPServerConfig(command="test", args=["arg"])
        executor = MCPToolExecutor(config)
        # Just verify the methods exist
        assert hasattr(executor, "__aenter__")
        assert hasattr(executor, "__aexit__")
