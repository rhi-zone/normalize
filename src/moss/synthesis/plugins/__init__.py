"""Synthesis Plugin Architecture.

This module provides the plugin infrastructure for moss synthesis,
enabling pluggable code generators, validators, and library learning.

Inspired by prior art:
- Synquid: Type-driven synthesis (SMTGenerator)
- miniKanren: Relational programming (RelationalStrategy)
- DreamCoder: Library learning (LibraryPlugin)
- lambda^2: Bidirectional type-example synthesis

Key components:
- CodeGenerator: Protocol for code generation plugins
- SynthesisValidator: Protocol for validation plugins
- LibraryPlugin: Protocol for abstraction libraries
- SynthesisRegistry: Unified registry for all synthesis plugins
"""

from .protocols import (
    Abstraction,
    CodeGenerator,
    GenerationCost,
    GenerationHints,
    GenerationResult,
    GeneratorMetadata,
    GeneratorType,
    LibraryMetadata,
    LibraryPlugin,
    SynthesisValidator,
    ValidationResult,
    ValidatorMetadata,
    ValidatorType,
)
from .registry import (
    GeneratorRegistry,
    LibraryRegistry,
    SynthesisRegistry,
    ValidatorRegistry,
    get_synthesis_registry,
    reset_synthesis_registry,
)

__all__ = [
    "Abstraction",
    "CodeGenerator",
    "GenerationCost",
    "GenerationHints",
    "GenerationResult",
    "GeneratorMetadata",
    "GeneratorRegistry",
    "GeneratorType",
    "LibraryMetadata",
    "LibraryPlugin",
    "LibraryRegistry",
    "SynthesisRegistry",
    "SynthesisValidator",
    "ValidationResult",
    "ValidatorMetadata",
    "ValidatorRegistry",
    "ValidatorType",
    "get_synthesis_registry",
    "reset_synthesis_registry",
]
