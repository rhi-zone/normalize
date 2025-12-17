"""Placeholder code generator.

Generates TODO placeholder code for specifications. This is the default
generator that provides minimal scaffolding.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from moss.synthesis.plugins.protocols import (
    CodeGenerator,
    GenerationCost,
    GenerationHints,
    GenerationResult,
    GeneratorMetadata,
    GeneratorType,
)

if TYPE_CHECKING:
    from moss.synthesis.types import Context, Specification


class PlaceholderGenerator:
    """Generator that returns placeholder TODO code.

    This is the default generator that provides minimal scaffolding
    when no other generator is available. Useful for:
    - Dry-run mode (see decomposition structure)
    - Development (stub out code for manual completion)
    - Fallback when specialized generators fail
    """

    def __init__(self) -> None:
        self._metadata = GeneratorMetadata(
            name="placeholder",
            generator_type=GeneratorType.PLACEHOLDER,
            priority=-100,  # Low priority, use as fallback
            description="Generates TODO placeholder code",
        )

    @property
    def metadata(self) -> GeneratorMetadata:
        """Return generator metadata."""
        return self._metadata

    def can_generate(self, spec: Specification, context: Context) -> bool:
        """Placeholder can always generate (fallback)."""
        return True

    async def generate(
        self,
        spec: Specification,
        context: Context,
        hints: GenerationHints | None = None,
    ) -> GenerationResult:
        """Generate placeholder code.

        Args:
            spec: The specification
            context: Available resources
            hints: Optional hints (ignored by placeholder)

        Returns:
            GenerationResult with placeholder code
        """
        # Check if already solved in context
        if spec.description in context.solved:
            return GenerationResult(
                success=True,
                code=str(context.solved[spec.description]),
                confidence=1.0,
                metadata={"source": "context.solved"},
            )

        # Check for primitive match
        for primitive in context.primitives:
            if primitive.lower() in spec.description.lower():
                return GenerationResult(
                    success=True,
                    code=primitive,
                    confidence=0.5,
                    metadata={"source": "primitive", "primitive": primitive},
                )

        # Generate placeholder
        lines = []
        lines.append(f"# Solution for: {spec.description}")

        if spec.type_signature:
            lines.append(f"# Type: {spec.type_signature}")

        if spec.constraints:
            lines.append("# Constraints:")
            for constraint in spec.constraints:
                lines.append(f"#   - {constraint}")

        if spec.examples:
            lines.append("# Examples:")
            for inp, out in spec.examples[:3]:  # Limit to 3 examples
                lines.append(f"#   {inp!r} -> {out!r}")

        lines.append("pass  # TODO: implement")

        code = "\n".join(lines) + "\n"

        return GenerationResult(
            success=True,
            code=code,
            confidence=0.0,  # Zero confidence - needs implementation
            metadata={"source": "placeholder"},
        )

    def estimate_cost(self, spec: Specification, context: Context) -> GenerationCost:
        """Placeholder generation is essentially free."""
        return GenerationCost(
            time_estimate_ms=1,
            token_estimate=0,
            complexity_score=0,
        )


# Protocol compliance check
assert isinstance(PlaceholderGenerator(), CodeGenerator)
