"""Tests for the driver architecture."""

import pytest

from moss.drivers import (
    Action,
    ActionResult,
    Context,
    DriverRegistry,
    LLMDriver,
    StateMachineDriver,
    StateTransition,
    UserDriver,
    WorkflowDriver,
    WorkflowState,
    WorkflowStep,
)


class MockSession:
    """Mock session for testing drivers."""

    def __init__(self, id: str = "test", description: str = "Test task"):
        self.id = id
        self.description = description
        self.project_root = None
        self._started = False
        self._completed = False
        self._failed = False
        self._error: str | None = None

    def start(self):
        self._started = True

    def complete(self):
        self._completed = True

    def fail(self, error: str):
        self._failed = True
        self._error = error


class TestAction:
    def test_action_str_no_params(self):
        action = Action(tool="view")
        assert str(action) == "view"

    def test_action_str_with_params(self):
        action = Action(tool="edit", parameters={"target": "foo.py"})
        assert "edit" in str(action)
        assert "foo.py" in str(action)


class TestContext:
    def test_empty_context(self):
        task = MockSession()
        ctx = Context(task=task)
        assert ctx.last_result() is None
        assert ctx.last_error() is None

    def test_context_with_history(self):
        task = MockSession()
        action = Action(tool="view")
        result = ActionResult(success=True, output="content")
        ctx = Context(task=task, history=[(action, result)])

        assert ctx.last_result() == result
        assert ctx.last_error() is None

    def test_context_last_error(self):
        task = MockSession()
        action = Action(tool="edit")
        result = ActionResult(success=False, error="Failed")
        ctx = Context(task=task, history=[(action, result)])

        assert ctx.last_error() == "Failed"


class TestDriverRegistry:
    def test_builtin_drivers_registered(self):
        drivers = DriverRegistry.list_drivers()
        assert "user" in drivers
        assert "llm" in drivers
        assert "workflow" in drivers
        assert "state_machine" in drivers

    def test_get_driver(self):
        assert DriverRegistry.get("user") is UserDriver
        assert DriverRegistry.get("llm") is LLMDriver
        assert DriverRegistry.get("workflow") is WorkflowDriver
        assert DriverRegistry.get("state_machine") is StateMachineDriver

    def test_get_unknown_driver(self):
        assert DriverRegistry.get("nonexistent") is None

    def test_create_driver(self):
        driver = DriverRegistry.create("user")
        assert isinstance(driver, UserDriver)

    def test_create_driver_with_config(self):
        driver = DriverRegistry.create("llm", model="custom-model")
        assert isinstance(driver, LLMDriver)
        assert driver.model == "custom-model"

    def test_create_unknown_driver_raises(self):
        with pytest.raises(ValueError, match="Unknown driver"):
            DriverRegistry.create("nonexistent")

    def test_register_custom_driver(self):
        class CustomDriver:
            name = "custom_test"

            async def decide_next_step(self, task, context):
                return None

        DriverRegistry.register(CustomDriver)
        assert DriverRegistry.get("custom_test") is CustomDriver


class TestUserDriver:
    @pytest.mark.asyncio
    async def test_no_prompt_returns_none(self):
        driver = UserDriver()
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is None

    @pytest.mark.asyncio
    async def test_done_command(self):
        async def prompt(*args):
            return "done"

        driver = UserDriver(prompt_callback=prompt)
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is None

    @pytest.mark.asyncio
    async def test_view_command(self):
        async def prompt(*args):
            return "view src/main.py"

        driver = UserDriver(prompt_callback=prompt)
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is not None
        assert action.tool == "view"
        assert action.parameters.get("target") == "src/main.py"

    @pytest.mark.asyncio
    async def test_shell_command(self):
        async def prompt(*args):
            return "shell ls -la"

        driver = UserDriver(prompt_callback=prompt)
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is not None
        assert action.tool == "shell"
        assert action.parameters.get("command") == "ls -la"


class TestWorkflowDriver:
    @pytest.mark.asyncio
    async def test_empty_workflow(self):
        driver = WorkflowDriver(steps=[])
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is None

    @pytest.mark.asyncio
    async def test_sequential_steps(self):
        steps = [
            WorkflowStep(tool="view", parameters={"target": "a.py"}),
            WorkflowStep(tool="view", parameters={"target": "b.py"}),
        ]
        driver = WorkflowDriver(steps=steps)
        task = MockSession()
        ctx = Context(task=task)

        action1 = await driver.decide_next_step(task, ctx)
        assert action1.parameters["target"] == "a.py"

        action2 = await driver.decide_next_step(task, ctx)
        assert action2.parameters["target"] == "b.py"

        action3 = await driver.decide_next_step(task, ctx)
        assert action3 is None

    @pytest.mark.asyncio
    async def test_dict_steps(self):
        steps = [{"tool": "view", "parameters": {"target": "foo.py"}}]
        driver = WorkflowDriver(steps=steps)
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action.tool == "view"
        assert action.parameters["target"] == "foo.py"

    @pytest.mark.asyncio
    async def test_conditional_step_true(self):
        steps = [
            WorkflowStep(tool="analyze", condition="last_success"),
        ]
        driver = WorkflowDriver(steps=steps)
        task = MockSession()

        # Context with successful last action
        prior_result = ActionResult(success=True)
        ctx = Context(task=task, history=[(Action(tool="view"), prior_result)])

        action = await driver.decide_next_step(task, ctx)
        assert action is not None
        assert action.tool == "analyze"

    @pytest.mark.asyncio
    async def test_conditional_step_false_skipped(self):
        steps = [
            WorkflowStep(tool="analyze", condition="last_success"),
            WorkflowStep(tool="view"),
        ]
        driver = WorkflowDriver(steps=steps)
        task = MockSession()

        # Context with failed last action
        prior_result = ActionResult(success=False, error="error")
        ctx = Context(task=task, history=[(Action(tool="edit"), prior_result)])

        action = await driver.decide_next_step(task, ctx)
        # Should skip analyze and return view
        assert action.tool == "view"


class TestStateMachineDriver:
    @pytest.mark.asyncio
    async def test_terminal_state(self):
        states = {
            "done": WorkflowState(name="done", terminal=True),
        }
        driver = StateMachineDriver(states=states, initial="done")
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action is None

    @pytest.mark.asyncio
    async def test_state_transitions(self):
        states = {
            "start": WorkflowState(
                name="start",
                action=Action(tool="view"),
                transitions=[
                    StateTransition(next_state="analyze", condition="success"),
                    StateTransition(next_state="error", condition="error"),
                ],
            ),
            "analyze": WorkflowState(
                name="analyze",
                action=Action(tool="analyze"),
                transitions=[StateTransition(next_state="done", condition="always")],
            ),
            "done": WorkflowState(name="done", terminal=True),
            "error": WorkflowState(name="error", terminal=True),
        }
        driver = StateMachineDriver(states=states, initial="start")
        task = MockSession()
        ctx = Context(task=task)

        # First step: start state
        action1 = await driver.decide_next_step(task, ctx)
        assert action1.tool == "view"

        # Complete with success -> should transition to analyze
        await driver.on_action_complete(task, action1, ActionResult(success=True))
        assert driver.current_state == "analyze"

        # Second step: analyze state
        action2 = await driver.decide_next_step(task, ctx)
        assert action2.tool == "analyze"

        # Complete -> should transition to done
        await driver.on_action_complete(task, action2, ActionResult(success=True))
        assert driver.current_state == "done"

        # Terminal state returns None
        action3 = await driver.decide_next_step(task, ctx)
        assert action3 is None

    @pytest.mark.asyncio
    async def test_dict_states(self):
        states = [
            {"name": "start", "action": Action(tool="view"), "terminal": False},
            {"name": "done", "terminal": True},
        ]
        driver = StateMachineDriver(states=states, initial="start")
        task = MockSession()
        ctx = Context(task=task)

        action = await driver.decide_next_step(task, ctx)
        assert action.tool == "view"


class TestLLMDriver:
    def test_default_system_prompt(self):
        driver = LLMDriver()
        assert "TOOL:" in driver.system_prompt
        assert "view" in driver.system_prompt

    def test_parse_response_done(self):
        driver = LLMDriver()
        result = driver._parse_response("TOOL: done")
        assert result is None

    def test_parse_response_action(self):
        driver = LLMDriver()
        result = driver._parse_response('TOOL: view\nPARAMS: {"target": "foo.py"}')
        assert result is not None
        assert result.tool == "view"
        assert result.parameters["target"] == "foo.py"

    def test_parse_response_no_params(self):
        driver = LLMDriver()
        result = driver._parse_response("TOOL: analyze")
        assert result is not None
        assert result.tool == "analyze"
        assert result.parameters == {}

    def test_parse_response_invalid(self):
        driver = LLMDriver()
        result = driver._parse_response("I think we should do something")
        # No TOOL: found, defaults to done
        assert result.tool == "done"
