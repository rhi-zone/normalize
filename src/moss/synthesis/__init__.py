"""Synthesis framework for recursive problem decomposition.

This module provides a domain-agnostic synthesis engine that integrates
with moss primitives (validation, shadow git, memory, events).

Core components:
- Specification: What to synthesize (description, types, examples, tests)
- Context: Available resources (primitives, library, solved problems)
- DecompositionStrategy: How to break problems into subproblems
- Composer: How to combine subproblem solutions
- StrategyRouter: Selects best strategy (like DWIM for tools)
- SynthesisFramework: Orchestrates the synthesis process

Example usage:
    from moss.synthesis import (
        SynthesisFramework,
        Specification,
        Context,
        create_synthesis_framework,
    )

    # Create framework
    framework = create_synthesis_framework()

    # Define specification
    spec = Specification(
        description="Sort a list of users by registration date",
        type_signature="List[User] -> List[User]",
    )

    # Define context
    context = Context(
        primitives=["sorted", "key", "lambda"],
        library={"User": User},
    )

    # Synthesize
    result = await framework.synthesize(spec, context)
    if result.success:
        print(result.solution)
"""

from .cache import (
    ExecutionResultCache,
    SolutionCache,
    StrategyCache,
    SynthesisCache,
    clear_all_caches,
    get_cache_stats,
    get_solution_cache,
    get_strategy_cache,
    get_test_cache,
)
from .composer import CodeComposer, Composer, FunctionComposer, SequentialComposer
from .framework import (
    SynthesisConfig,
    SynthesisEventType,
    SynthesisFramework,
    SynthesisState,
    create_synthesis_framework,
)
from .presets import (
    PresetName,
    SynthesisPreset,
    get_preset,
    get_preset_descriptions,
    list_presets,
    register_preset,
)
from .router import StrategyMatch, StrategyRouter
from .strategy import AtomicStrategy, DecompositionStrategy, StrategyMetadata
from .types import (
    CompositionError,
    Context,
    DecompositionError,
    NoStrategyError,
    Specification,
    Subproblem,
    SynthesisError,
    SynthesisResult,
    ValidationError,
)

__all__ = [
    "AtomicStrategy",
    "CodeComposer",
    "Composer",
    "CompositionError",
    "Context",
    "DecompositionError",
    "DecompositionStrategy",
    "ExecutionResultCache",
    "FunctionComposer",
    "NoStrategyError",
    "PresetName",
    "SequentialComposer",
    "SolutionCache",
    "Specification",
    "StrategyCache",
    "StrategyMatch",
    "StrategyMetadata",
    "StrategyRouter",
    "Subproblem",
    "SynthesisCache",
    "SynthesisConfig",
    "SynthesisError",
    "SynthesisEventType",
    "SynthesisFramework",
    "SynthesisPreset",
    "SynthesisResult",
    "SynthesisState",
    "ValidationError",
    "clear_all_caches",
    "create_synthesis_framework",
    "get_cache_stats",
    "get_preset",
    "get_preset_descriptions",
    "get_solution_cache",
    "get_strategy_cache",
    "get_test_cache",
    "list_presets",
    "register_preset",
]
