"""Example Python workflows demonstrating dynamic step generation.

These workflows show how to create workflows with conditional logic that
can't be expressed in static TOML format.
"""

from dataclasses import dataclass, field

from moss.workflows import (
    Workflow,
    WorkflowContext,
    WorkflowLimits,
    WorkflowLLMConfig,
    WorkflowStep,
)


@dataclass
class ConditionalTestWorkflow(Workflow):
    """Workflow that adds test step only if project has tests.

    Example of context-aware dynamic step generation.
    """

    name: str = "conditional-test"
    description: str = "Validate and optionally run tests based on project structure"
    version: str = "1.0"
    limits: WorkflowLimits = field(default_factory=WorkflowLimits)
    llm: WorkflowLLMConfig = field(default_factory=WorkflowLLMConfig)
    steps: list[WorkflowStep] = field(default_factory=list)

    def build_steps(self, context: WorkflowContext | None = None) -> list[WorkflowStep]:
        """Build steps, adding test step if project has tests."""
        steps = [
            WorkflowStep(name="validate", tool="validator.run", on_error="skip"),
            WorkflowStep(
                name="analyze",
                tool="llm.analyze",
                type="llm",
                input_from="validate",
            ),
            WorkflowStep(
                name="fix",
                tool="patch.apply",
                input_from="analyze",
                max_retries=3,
            ),
        ]

        # Add test step if project has tests
        if context and context.has_tests:
            steps.append(
                WorkflowStep(
                    name="test",
                    tool="pytest.run",
                    input_from="fix",
                    on_error="skip",
                )
            )

        return steps


@dataclass
class LanguageAwareWorkflow(Workflow):
    """Workflow that adapts to the target language.

    Uses different validation tools based on file extension.
    """

    name: str = "language-aware"
    description: str = "Validate using language-appropriate tools"
    version: str = "1.0"
    limits: WorkflowLimits = field(default_factory=WorkflowLimits)
    llm: WorkflowLLMConfig = field(default_factory=WorkflowLLMConfig)
    steps: list[WorkflowStep] = field(default_factory=list)

    # Language to validator mapping
    _validators: dict[str, str] = field(
        default_factory=lambda: {
            "python": "ruff.check",
            "rust": "cargo.check",
            "typescript": "tsc.check",
            "javascript": "eslint.check",
            "go": "go.vet",
        }
    )

    def build_steps(self, context: WorkflowContext | None = None) -> list[WorkflowStep]:
        """Build steps with language-specific validator."""
        lang = context.language if context else "python"
        validator = self._validators.get(lang, "validator.run")

        return [
            WorkflowStep(name="validate", tool=validator, on_error="skip"),
            WorkflowStep(
                name="analyze",
                tool="llm.analyze",
                type="llm",
                input_from="validate",
            ),
            WorkflowStep(
                name="fix",
                tool="patch.apply",
                input_from="analyze",
                max_retries=3,
            ),
        ]
