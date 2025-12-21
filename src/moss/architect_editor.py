"""Architect/Editor Split: Separate reasoning from editing.

Design principle: High-level reasoning (what to change) is separate from
low-level execution (how to change it).

Benefits:
- Architect uses expensive model for planning, Editor can use cheaper/faster model
- Clear separation makes debugging and testing easier
- Better for complex multi-step edits
- Reduces token waste by not re-reasoning in edit phase

Usage:
    architect = LLMArchitect(llm_config)
    editor = StructuredEditor(api)
    loop = ArchitectEditorLoop(architect, editor)
    result = await loop.run(task, file_path)
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from enum import Enum, auto
from pathlib import Path
from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

if TYPE_CHECKING:
    from moss.moss_api import MossAPI

logger = logging.getLogger(__name__)


class EditType(Enum):
    """Type of edit operation."""

    REPLACE = auto()  # Replace content at location
    INSERT_BEFORE = auto()  # Insert before location
    INSERT_AFTER = auto()  # Insert after location
    DELETE = auto()  # Delete at location
    RENAME = auto()  # Rename symbol


@dataclass
class EditStep:
    """Single atomic edit operation.

    This is what the Architect produces - a structured description of
    what to change, where, and why.
    """

    edit_type: EditType
    target: str  # Symbol name, line range, or anchor
    new_content: str | None = None  # For replace/insert
    reason: str = ""  # Why this change (for debugging/review)
    depends_on: list[int] = field(default_factory=list)  # Step indices this depends on

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "edit_type": self.edit_type.name.lower(),
            "target": self.target,
            "new_content": self.new_content,
            "reason": self.reason,
            "depends_on": self.depends_on,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> EditStep:
        """Create from dictionary."""
        return cls(
            edit_type=EditType[data["edit_type"].upper()],
            target=data["target"],
            new_content=data.get("new_content"),
            reason=data.get("reason", ""),
            depends_on=data.get("depends_on", []),
        )


@dataclass
class EditPlan:
    """Plan produced by Architect - what changes to make.

    This is the structured output of the reasoning phase. It describes
    the high-level approach without implementing the actual edits.
    """

    task: str  # Original task description
    file_path: str  # Target file
    approach: str  # High-level approach description
    steps: list[EditStep]  # Ordered list of edits to make
    context_needed: list[str] = field(default_factory=list)  # Symbols to expand before editing
    risks: list[str] = field(default_factory=list)  # Potential issues to watch for
    success_criteria: str = ""  # How to know the edit succeeded

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "task": self.task,
            "file_path": self.file_path,
            "approach": self.approach,
            "steps": [s.to_dict() for s in self.steps],
            "context_needed": self.context_needed,
            "risks": self.risks,
            "success_criteria": self.success_criteria,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> EditPlan:
        """Create from dictionary."""
        return cls(
            task=data["task"],
            file_path=data["file_path"],
            approach=data["approach"],
            steps=[EditStep.from_dict(s) for s in data["steps"]],
            context_needed=data.get("context_needed", []),
            risks=data.get("risks", []),
            success_criteria=data.get("success_criteria", ""),
        )


class EditStatus(Enum):
    """Status of an edit step execution."""

    PENDING = auto()
    SUCCESS = auto()
    FAILED = auto()
    SKIPPED = auto()


@dataclass
class EditResult:
    """Result of executing a single edit step."""

    step_index: int
    status: EditStatus
    output: Any = None
    error: str | None = None
    tokens_used: int = 0


@dataclass
class LoopResult:
    """Result of running the Architect/Editor loop."""

    success: bool
    plan: EditPlan | None = None
    results: list[EditResult] = field(default_factory=list)
    validation_output: Any = None
    error: str | None = None
    total_tokens: int = 0
    iterations: int = 0

    def to_compact(self) -> str:
        """Format as compact summary."""
        status = "✓" if self.success else "✗"
        steps = len(self.results) if self.results else 0
        succeeded = sum(1 for r in self.results if r.status == EditStatus.SUCCESS)
        return (
            f"{status} {succeeded}/{steps} steps | "
            f"{self.total_tokens} tokens | {self.iterations} iterations"
        )


@runtime_checkable
class Architect(Protocol):
    """Protocol for the planning phase.

    The Architect analyzes the task and produces a structured EditPlan
    describing what changes to make.
    """

    async def plan(
        self,
        task: str,
        file_path: str,
        skeleton: str,
        expanded_context: dict[str, str] | None = None,
    ) -> EditPlan:
        """Create an edit plan for the given task.

        Args:
            task: Description of what to do
            file_path: Target file path
            skeleton: File skeleton for context
            expanded_context: Optional dict of symbol -> full code

        Returns:
            EditPlan with structured steps
        """
        ...

    async def revise(
        self,
        plan: EditPlan,
        validation_errors: list[str],
        attempt: int,
    ) -> EditPlan:
        """Revise a plan based on validation errors.

        Args:
            plan: Original plan that failed validation
            validation_errors: List of errors to fix
            attempt: Which revision attempt (1, 2, ...)

        Returns:
            Revised EditPlan
        """
        ...


@runtime_checkable
class Editor(Protocol):
    """Protocol for the execution phase.

    The Editor takes an EditPlan and executes each step, applying
    the actual changes to the file.
    """

    async def execute(self, plan: EditPlan) -> list[EditResult]:
        """Execute all steps in the edit plan.

        Args:
            plan: The edit plan to execute

        Returns:
            List of results for each step
        """
        ...


class LLMArchitect:
    """Architect that uses LLM for planning.

    Uses structured output to produce EditPlan from natural language task.
    """

    def __init__(
        self,
        model: str = "gemini/gemini-3-flash-preview",
        temperature: float = 0.0,
        mock: bool = False,
    ):
        self.model = model
        self.temperature = temperature
        self.mock = mock

    async def plan(
        self,
        task: str,
        file_path: str,
        skeleton: str,
        expanded_context: dict[str, str] | None = None,
    ) -> EditPlan:
        """Create an edit plan using LLM reasoning."""
        if self.mock:
            return self._mock_plan(task, file_path)

        prompt = self._build_plan_prompt(task, file_path, skeleton, expanded_context)
        response = await self._call_llm(prompt)
        return self._parse_plan_response(response, task, file_path)

    async def revise(
        self,
        plan: EditPlan,
        validation_errors: list[str],
        attempt: int,
    ) -> EditPlan:
        """Revise plan based on validation errors."""
        if self.mock:
            return plan

        prompt = self._build_revise_prompt(plan, validation_errors, attempt)
        response = await self._call_llm(prompt)
        return self._parse_plan_response(response, plan.task, plan.file_path)

    def _build_plan_prompt(
        self,
        task: str,
        file_path: str,
        skeleton: str,
        expanded_context: dict[str, str] | None,
    ) -> str:
        """Build the planning prompt."""
        context_section = ""
        if expanded_context:
            context_section = "\n\nExpanded code:\n"
            for symbol, code in expanded_context.items():
                context_section += f"\n{symbol}:\n{code}\n"

        return f"""Plan edits for this task. Output structured plan.

Task: {task}
File: {file_path}

Skeleton:
{skeleton}
{context_section}
Output format:
APPROACH: One sentence describing the high-level approach
CONTEXT_NEEDED: comma-separated symbols to expand (or "none")
RISKS: comma-separated potential issues (or "none")
SUCCESS_CRITERIA: How to verify the edit worked

STEPS:
1. TYPE:replace TARGET:function_name REASON:why
   CONTENT:
   new code here
2. TYPE:insert_after TARGET:symbol REASON:why
   CONTENT:
   code to insert

Valid types: replace, insert_before, insert_after, delete, rename
"""

    def _build_revise_prompt(
        self,
        plan: EditPlan,
        validation_errors: list[str],
        attempt: int,
    ) -> str:
        """Build the revision prompt."""
        errors = "\n".join(f"- {e}" for e in validation_errors)
        return f"""Revise the edit plan to fix these errors. Attempt {attempt}.

Original task: {plan.task}
File: {plan.file_path}

Previous approach: {plan.approach}

Validation errors:
{errors}

Create a revised plan that addresses these errors.
Use the same output format as before.
"""

    def _mock_plan(self, task: str, file_path: str) -> EditPlan:
        """Return a mock plan for testing."""
        return EditPlan(
            task=task,
            file_path=file_path,
            approach="Mock approach for testing",
            steps=[
                EditStep(
                    edit_type=EditType.REPLACE,
                    target="mock_function",
                    new_content="def mock_function(): pass",
                    reason="Mock edit for testing",
                )
            ],
            success_criteria="Tests pass",
        )

    async def _call_llm(self, prompt: str) -> str:
        """Call LLM and return response."""
        import asyncio

        try:
            from litellm import completion
        except ImportError as e:
            raise ImportError(
                "litellm required for LLMArchitect. Install with: pip install 'moss[llm]'"
            ) from e

        def _sync_call() -> str:
            response = completion(
                model=self.model,
                messages=[
                    {
                        "role": "system",
                        "content": "You are a code editing planner. Be terse and precise.",
                    },
                    {"role": "user", "content": prompt},
                ],
                temperature=self.temperature,
            )
            return response.choices[0].message.content or ""

        return await asyncio.to_thread(_sync_call)

    def _parse_plan_response(self, response: str, task: str, file_path: str) -> EditPlan:
        """Parse LLM response into EditPlan."""
        lines = response.strip().split("\n")

        approach = ""
        context_needed: list[str] = []
        risks: list[str] = []
        success_criteria = ""
        steps: list[EditStep] = []

        current_step: dict[str, Any] | None = None
        in_content = False
        content_lines: list[str] = []

        for line in lines:
            stripped = line.strip()
            upper = stripped.upper()

            # Parse headers
            if upper.startswith("APPROACH:"):
                approach = stripped[9:].strip()
                continue
            elif upper.startswith("CONTEXT_NEEDED:"):
                val = stripped[15:].strip()
                if val.lower() != "none":
                    context_needed = [s.strip() for s in val.split(",") if s.strip()]
                continue
            elif upper.startswith("RISKS:"):
                val = stripped[6:].strip()
                if val.lower() != "none":
                    risks = [s.strip() for s in val.split(",") if s.strip()]
                continue
            elif upper.startswith("SUCCESS_CRITERIA:"):
                success_criteria = stripped[17:].strip()
                continue
            elif upper.startswith("STEPS:"):
                continue

            # Parse steps
            if stripped and stripped[0].isdigit() and "." in stripped[:3]:
                # Save previous step
                if current_step is not None:
                    current_step["content"] = "\n".join(content_lines).strip()
                    steps.append(self._dict_to_step(current_step))

                # Start new step
                current_step = self._parse_step_header(stripped)
                content_lines = []
                in_content = False
            elif upper.startswith("CONTENT:"):
                in_content = True
                rest = stripped[8:].strip()
                if rest:
                    content_lines.append(rest)
            elif in_content:
                content_lines.append(line)
            elif current_step is not None and ":" in stripped:
                # Additional step attributes
                key, val = stripped.split(":", 1)
                key = key.strip().upper()
                if key == "TYPE":
                    current_step["type"] = val.strip().lower()
                elif key == "TARGET":
                    current_step["target"] = val.strip()
                elif key == "REASON":
                    current_step["reason"] = val.strip()

        # Save last step
        if current_step is not None:
            current_step["content"] = "\n".join(content_lines).strip()
            steps.append(self._dict_to_step(current_step))

        return EditPlan(
            task=task,
            file_path=file_path,
            approach=approach,
            steps=steps,
            context_needed=context_needed,
            risks=risks,
            success_criteria=success_criteria,
        )

    def _parse_step_header(self, line: str) -> dict[str, Any]:
        """Parse a step header line like '1. TYPE:replace TARGET:foo REASON:bar'."""
        # Remove step number
        rest = line.split(".", 1)[1].strip() if "." in line else line

        step: dict[str, Any] = {"type": "replace", "target": "", "reason": "", "content": ""}

        # Parse key:value pairs
        parts = rest.split()
        for part in parts:
            if ":" in part:
                key, val = part.split(":", 1)
                key = key.upper()
                if key == "TYPE":
                    step["type"] = val.lower()
                elif key == "TARGET":
                    step["target"] = val

        # REASON might be the rest of the line after other parts
        if "REASON:" in rest.upper():
            idx = rest.upper().find("REASON:")
            step["reason"] = rest[idx + 7 :].strip()

        return step

    def _dict_to_step(self, d: dict[str, Any]) -> EditStep:
        """Convert dict to EditStep."""
        type_map = {
            "replace": EditType.REPLACE,
            "insert_before": EditType.INSERT_BEFORE,
            "insert_after": EditType.INSERT_AFTER,
            "delete": EditType.DELETE,
            "rename": EditType.RENAME,
        }
        return EditStep(
            edit_type=type_map.get(d.get("type", "replace"), EditType.REPLACE),
            target=d.get("target", ""),
            new_content=d.get("content") or None,
            reason=d.get("reason", ""),
        )


class StructuredEditor:
    """Editor that applies structured edits using MossAPI.

    Takes an EditPlan and executes each step in order, using
    patch.apply or anchor-based editing.
    """

    def __init__(self, api: MossAPI | None = None, root: Path | str | None = None):
        """Initialize the editor.

        Args:
            api: MossAPI instance (created if None)
            root: Project root for creating MossAPI
        """
        self._api = api
        self._root = Path(root) if root else None

    @property
    def api(self) -> MossAPI:
        """Lazy-initialize MossAPI."""
        if self._api is None:
            from moss.moss_api import MossAPI

            self._api = MossAPI(self._root or Path.cwd())
        return self._api

    async def execute(self, plan: EditPlan) -> list[EditResult]:
        """Execute all steps in the edit plan."""
        results: list[EditResult] = []

        for idx, step in enumerate(plan.steps):
            # Check dependencies
            if step.depends_on:
                for dep_idx in step.depends_on:
                    if dep_idx < len(results) and results[dep_idx].status != EditStatus.SUCCESS:
                        results.append(
                            EditResult(
                                step_index=idx,
                                status=EditStatus.SKIPPED,
                                error=f"Dependency step {dep_idx} failed",
                            )
                        )
                        continue

            result = await self._execute_step(plan.file_path, step, idx)
            results.append(result)

        return results

    async def _execute_step(self, file_path: str, step: EditStep, idx: int) -> EditResult:
        """Execute a single edit step."""
        try:
            if step.edit_type == EditType.REPLACE:
                output = await self._replace(file_path, step.target, step.new_content or "")
            elif step.edit_type == EditType.INSERT_BEFORE:
                output = await self._insert_before(file_path, step.target, step.new_content or "")
            elif step.edit_type == EditType.INSERT_AFTER:
                output = await self._insert_after(file_path, step.target, step.new_content or "")
            elif step.edit_type == EditType.DELETE:
                output = await self._delete(file_path, step.target)
            elif step.edit_type == EditType.RENAME:
                output = await self._rename(file_path, step.target, step.new_content or "")
            else:
                return EditResult(
                    step_index=idx,
                    status=EditStatus.FAILED,
                    error=f"Unknown edit type: {step.edit_type}",
                )

            return EditResult(step_index=idx, status=EditStatus.SUCCESS, output=output)

        except Exception as e:
            logger.warning(f"Edit step {idx} failed: {e}")
            return EditResult(step_index=idx, status=EditStatus.FAILED, error=str(e))

    async def _replace(self, file_path: str, target: str, content: str) -> Any:
        """Replace a symbol's content."""
        from moss.anchors import Anchor
        from moss.patches import Patch, PatchType

        anchor = Anchor(name=target)
        patch = Patch(anchor=anchor, patch_type=PatchType.REPLACE, content=content)
        return self.api.patch.apply(file_path, patch)

    async def _insert_before(self, file_path: str, target: str, content: str) -> Any:
        """Insert content before a symbol."""
        from moss.anchors import Anchor
        from moss.patches import Patch, PatchType

        anchor = Anchor(name=target)
        patch = Patch(anchor=anchor, patch_type=PatchType.INSERT_BEFORE, content=content)
        return self.api.patch.apply(file_path, patch)

    async def _insert_after(self, file_path: str, target: str, content: str) -> Any:
        """Insert content after a symbol."""
        from moss.anchors import Anchor
        from moss.patches import Patch, PatchType

        anchor = Anchor(name=target)
        patch = Patch(anchor=anchor, patch_type=PatchType.INSERT_AFTER, content=content)
        return self.api.patch.apply(file_path, patch)

    async def _delete(self, file_path: str, target: str) -> Any:
        """Delete a symbol."""
        from moss.anchors import Anchor
        from moss.patches import Patch, PatchType

        anchor = Anchor(name=target)
        patch = Patch(anchor=anchor, patch_type=PatchType.DELETE, content="")
        return self.api.patch.apply(file_path, patch)

    async def _rename(self, file_path: str, target: str, new_name: str) -> Any:
        """Rename a symbol (placeholder - needs refactor support)."""
        raise NotImplementedError("Rename requires refactoring support")


class ArchitectEditorLoop:
    """Main loop that orchestrates Architect and Editor.

    Flow:
    1. Get file skeleton for context
    2. Optionally expand symbols Architect requests
    3. Architect creates EditPlan
    4. Editor executes EditPlan
    5. Validate result
    6. If errors, Architect revises plan, goto 4
    7. Return success or failure after max iterations
    """

    def __init__(
        self,
        architect: Architect,
        editor: Editor,
        api: MossAPI | None = None,
        max_iterations: int = 3,
    ):
        self.architect = architect
        self.editor = editor
        self._api = api
        self.max_iterations = max_iterations

    @property
    def api(self) -> MossAPI:
        """Lazy-initialize MossAPI."""
        if self._api is None:
            from moss.moss_api import MossAPI

            self._api = MossAPI(Path.cwd())
        return self._api

    async def run(self, task: str, file_path: str) -> LoopResult:
        """Run the Architect/Editor loop.

        Args:
            task: What to do (natural language)
            file_path: Target file path

        Returns:
            LoopResult with success status and details
        """
        total_tokens = 0
        iteration = 0

        # Get skeleton for context
        try:
            skeleton = self.api.skeleton.format(file_path)
        except (OSError, ValueError) as e:
            return LoopResult(
                success=False,
                error=f"Failed to get skeleton: {e}",
                iterations=0,
            )

        # Initial planning
        try:
            plan = await self.architect.plan(task, file_path, skeleton)
        except Exception as e:
            return LoopResult(
                success=False,
                error=f"Architect failed to plan: {e}",
                iterations=0,
            )

        # Expand context if requested
        expanded_context: dict[str, str] = {}
        if plan.context_needed:
            for symbol in plan.context_needed:
                try:
                    expanded = self.api.skeleton.expand(file_path, symbol)
                    if expanded:
                        expanded_context[symbol] = expanded
                except (OSError, ValueError):
                    pass  # Continue without this context

            # Re-plan with expanded context
            if expanded_context:
                try:
                    plan = await self.architect.plan(task, file_path, skeleton, expanded_context)
                except Exception as e:
                    return LoopResult(
                        success=False,
                        plan=plan,
                        error=f"Architect failed to re-plan with context: {e}",
                        iterations=0,
                    )

        # Edit-validate loop
        all_results: list[EditResult] = []
        validation_output: Any = None

        while iteration < self.max_iterations:
            iteration += 1

            # Execute the plan
            results = await self.editor.execute(plan)
            all_results.extend(results)

            # Check if any step failed
            failed_steps = [r for r in results if r.status == EditStatus.FAILED]
            if failed_steps:
                errors = [r.error or "Unknown error" for r in failed_steps]
                try:
                    plan = await self.architect.revise(plan, errors, iteration)
                except Exception as e:
                    return LoopResult(
                        success=False,
                        plan=plan,
                        results=all_results,
                        error=f"Architect failed to revise: {e}",
                        total_tokens=total_tokens,
                        iterations=iteration,
                    )
                continue

            # Validate the result
            try:
                validation_output = self.api.validation.validate(file_path)
            except (OSError, ValueError) as e:
                return LoopResult(
                    success=False,
                    plan=plan,
                    results=all_results,
                    error=f"Validation failed: {e}",
                    total_tokens=total_tokens,
                    iterations=iteration,
                )

            # Check validation result
            is_valid = self._check_validation(validation_output)
            if is_valid:
                return LoopResult(
                    success=True,
                    plan=plan,
                    results=all_results,
                    validation_output=validation_output,
                    total_tokens=total_tokens,
                    iterations=iteration,
                )

            # Validation failed - revise plan
            errors = self._extract_validation_errors(validation_output)
            try:
                plan = await self.architect.revise(plan, errors, iteration)
            except Exception as e:
                return LoopResult(
                    success=False,
                    plan=plan,
                    results=all_results,
                    validation_output=validation_output,
                    error=f"Architect failed to revise after validation: {e}",
                    total_tokens=total_tokens,
                    iterations=iteration,
                )

        # Max iterations reached
        return LoopResult(
            success=False,
            plan=plan,
            results=all_results,
            validation_output=validation_output,
            error=f"Max iterations ({self.max_iterations}) reached",
            total_tokens=total_tokens,
            iterations=iteration,
        )

    def _check_validation(self, result: Any) -> bool:
        """Check if validation passed."""
        if hasattr(result, "success"):
            return bool(result.success)
        if isinstance(result, dict):
            return result.get("success", False)
        return True  # Assume success if unknown format

    def _extract_validation_errors(self, result: Any) -> list[str]:
        """Extract error messages from validation result."""
        errors: list[str] = []

        if hasattr(result, "issues"):
            for issue in result.issues:
                errors.append(str(issue))
        elif isinstance(result, dict):
            for issue in result.get("issues", []):
                if isinstance(issue, dict):
                    errors.append(issue.get("message", str(issue)))
                else:
                    errors.append(str(issue))

        return errors[:10]  # Limit to 10 errors


async def run_architect_editor(
    task: str,
    file_path: str,
    model: str = "gemini/gemini-3-flash-preview",
    mock: bool = False,
) -> LoopResult:
    """Convenience function to run the Architect/Editor loop.

    Args:
        task: What to do (natural language)
        file_path: Target file path
        model: LLM model for Architect
        mock: If True, use mock responses

    Returns:
        LoopResult with success status and details
    """
    architect = LLMArchitect(model=model, mock=mock)
    editor = StructuredEditor()
    loop = ArchitectEditorLoop(architect, editor)
    return await loop.run(task, file_path)
