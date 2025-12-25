"""Command-line interface for Moss.

# See: docs/cli/commands.md
"""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path
from typing import TYPE_CHECKING, Any

from moss_orchestration.output import Output, Verbosity, configure_output, get_output

if TYPE_CHECKING:
    from argparse import Namespace


def get_version() -> str:
    """Get the moss version."""
    from moss_cli import __version__

    return __version__


def setup_output(args: Namespace) -> Output:
    """Configure global output based on CLI args."""
    # Determine verbosity
    if getattr(args, "quiet", False):
        verbosity = Verbosity.QUIET
    elif getattr(args, "debug", False):
        verbosity = Verbosity.DEBUG
    elif getattr(args, "verbose", False):
        verbosity = Verbosity.VERBOSE
    else:
        verbosity = Verbosity.NORMAL

    # Determine compact mode
    # Explicit --compact always wins, otherwise default to compact when not a TTY
    compact = getattr(args, "compact", False)
    json_format = getattr(args, "json", False)
    if not compact and not json_format:
        compact = not sys.stdout.isatty()

    # Configure output
    output = configure_output(
        verbosity=verbosity,
        json_format=json_format,
        compact=compact,
        no_color=getattr(args, "no_color", False),
        jq_expr=getattr(args, "jq", None),
    )

    return output


def wants_json(args: Namespace) -> bool:
    """Check if JSON output is requested (via --json or --jq)."""
    return getattr(args, "json", False) or getattr(args, "jq", None) is not None


def output_result(data: Any, args: Namespace) -> None:
    """Output result in appropriate format."""
    output = get_output()
    output.data(data)


def cmd_init(args: Namespace) -> int:
    """Initialize a new moss project."""
    output = setup_output(args)
    project_dir = Path(args.directory).resolve()

    if not project_dir.exists():
        output.error(f"Directory {project_dir} does not exist")
        return 1

    config_file = project_dir / "moss_config.py"

    if config_file.exists() and not args.force:
        output.error(f"{config_file} already exists. Use --force to overwrite.")
        return 1

    # Determine distro
    distro_name = args.distro or "python"

    config_content = f'''"""Moss configuration for this project."""

from pathlib import Path

from moss_cli.config import MossConfig, get_distro

# Start from a base distro
base = get_distro("{distro_name}")
config = base.create_config() if base else MossConfig()

# Configure for this project
config = (
    config
    .with_project(Path(__file__).parent, "{project_dir.name}")
    .with_validators(syntax=True, ruff=True, pytest=False)
    .with_policies(velocity=True, quarantine=True, path=True)
    .with_loop(max_iterations=10, auto_commit=True)
)

# Add static context files (architecture docs, etc.)
# config = config.with_static_context(Path("docs/architecture/overview.md"))

# Add custom validators
# from moss.validators import CommandValidator
# config = config.add_validator(CommandValidator("mypy", ["mypy", "."]))
'''

    config_file.write_text(config_content)
    output.success(f"Created {config_file}")

    # Create .moss directory for runtime data
    moss_dir = project_dir / ".moss"
    if not moss_dir.exists():
        moss_dir.mkdir()
        (moss_dir / ".gitignore").write_text("*\n")
        output.verbose(f"Created {moss_dir}/")

    output.info(f"Moss initialized in {project_dir}")
    output.info(f"  Config: {config_file.name}")
    output.info(f"  Distro: {distro_name}")
    output.blank()
    output.step("Next steps:")
    output.info("  1. Edit moss_config.py to customize your configuration")
    output.info("  2. Run 'moss run \"your task\"' to execute a task")

    return 0


def cmd_run(args: Namespace) -> int:
    """Run a task through moss."""
    from moss_cli.config import load_config_file
    from moss_orchestration.agents import create_manager
    from moss_orchestration.events import EventBus
    from moss_orchestration.shadow_git import ShadowGit
    from moss_orchestration.task_api import TaskRequest, create_api_handler

    output = setup_output(args)
    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    # Load config
    if config_file.exists():
        try:
            load_config_file(config_file)
        except Exception as e:
            output.error(f"Error loading config: {e}")
            return 1

    # Set up components
    event_bus = EventBus()

    # Listen for tool calls to show metrics
    def on_tool_call(event: Any) -> None:
        tool = event.payload.get("tool_name", "unknown")
        success = event.payload.get("success", True)
        duration = event.payload.get("duration_ms", 0)
        mem = event.payload.get("memory_bytes", 0) / 1024 / 1024
        ctx = event.payload.get("context_tokens", 0)
        breakdown = event.payload.get("memory_breakdown", {})

        # Format breakdown
        bd_str = ""
        if breakdown:
            sorted_bd = sorted(breakdown.items(), key=lambda x: x[1], reverse=True)
            bd_parts = [f"{k}={v / 1024 / 1024:.1f}MB" for k, v in sorted_bd[:2]]
            bd_str = f" [{', '.join(bd_parts)}]"

        status = "✓" if success else "✗"
        output.info(
            f"  {status} {tool} ({duration}ms) | RAM: {mem:.1f} MB{bd_str} | Context: {ctx} tokens"
        )

    from moss_orchestration.events import EventType

    event_bus.subscribe(EventType.TOOL_CALL, on_tool_call)

    shadow_git = ShadowGit(project_dir)
    manager = create_manager(shadow_git, event_bus)
    handler = create_api_handler(manager, event_bus)

    # Create task request
    request = TaskRequest(
        task=args.task,
        priority=args.priority,
        constraints=args.constraint or [],
    )

    async def run_task() -> int:
        response = await handler.create_task(request)
        output.success(f"Task created: {response.request_id}")
        output.info(f"Ticket: {response.ticket_id}")
        output.info(f"Status: {response.status.value}")

        if args.wait:
            output.step("Waiting for completion...")
            # Poll for status
            while True:
                status = await handler.get_task_status(response.request_id)
                if status is None:
                    output.error("Task not found")
                    return 1

                if status.status.value in ("COMPLETED", "FAILED", "CANCELLED"):
                    if status.status.value == "COMPLETED":
                        output.success(f"Final status: {status.status.value}")
                    else:
                        output.warning(f"Final status: {status.status.value}")
                    if status.result:
                        output.data(status.result)
                    break

                await asyncio.sleep(0.5)

        return 0

    return asyncio.run(run_task())


def cmd_status(args: Namespace) -> int:
    """Show status of moss tasks and workers."""
    from moss_cli.config import load_config_file
    from moss_orchestration.agents import create_manager
    from moss_orchestration.events import EventBus
    from moss_orchestration.shadow_git import ShadowGit
    from moss_orchestration.task_api import create_api_handler

    output = setup_output(args)
    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    # Load config (validates it's readable)
    if config_file.exists():
        try:
            load_config_file(config_file)
        except Exception as e:
            output.error(f"Error loading config: {e}")
            return 1

    # Set up components
    event_bus = EventBus()
    shadow_git = ShadowGit(project_dir)
    manager = create_manager(shadow_git, event_bus)
    handler = create_api_handler(manager, event_bus)

    # Get stats
    stats = handler.get_stats()

    if getattr(args, "json", False):
        output_result(stats, args)
        return 0

    output.header("Moss Status")
    output.info(f"Project: {project_dir.name}")
    output.info(f"Config: {'moss_config.py' if config_file.exists() else '(default)'}")
    output.blank()
    output.step("API Handler:")
    output.info(f"  Active requests: {stats['active_requests']}")
    output.info(f"  Pending checkpoints: {stats['pending_checkpoints']}")
    output.info(f"  Active streams: {stats['active_streams']}")
    output.blank()
    output.step("Manager:")
    manager_stats = stats["manager_stats"]
    output.info(f"  Active workers: {manager_stats['active_workers']}")
    output.info(f"  Total tickets: {manager_stats['total_tickets']}")
    tickets_by_status = manager_stats.get("tickets_by_status", {})
    if tickets_by_status:
        output.info(f"  Tickets by status: {tickets_by_status}")

    # Show verbose info using output.verbose()
    output.verbose("Workers:")
    for worker_id, worker_info in manager_stats.get("workers", {}).items():
        output.verbose(f"  {worker_id}: {worker_info}")

    return 0


def cmd_config(args: Namespace) -> int:
    """Show or validate configuration."""
    from moss_cli.config import list_distros, load_config_file

    output = setup_output(args)

    if args.list_distros:
        output.info("Available distros:")
        for name in list_distros():
            output.info(f"  - {name}")
        return 0

    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    if not config_file.exists():
        output.error(f"No config file found at {config_file}")
        output.info("Run 'moss init' to create one.")
        return 1

    try:
        config = load_config_file(config_file)
    except Exception as e:
        output.error(f"Error loading config: {e}")
        return 1

    if args.validate:
        errors = config.validate()
        if errors:
            output.error("Configuration errors:")
            for error in errors:
                output.error(f"  - {error}")
            return 1
        output.success("Configuration is valid.")
        return 0

    # Show config
    output.header("Configuration")
    output.info(f"Project: {config.project_name}")
    output.info(f"Root: {config.project_root}")
    output.info(f"Extends: {', '.join(config.extends) or '(none)'}")
    output.blank()
    output.step("Validators:")
    output.info(f"  syntax: {config.validators.syntax}")
    output.info(f"  ruff: {config.validators.ruff}")
    output.info(f"  pytest: {config.validators.pytest}")
    output.info(f"  custom: {len(config.validators.custom)}")
    output.blank()
    output.step("Policies:")
    output.info(f"  velocity: {config.policies.velocity}")
    output.info(f"  quarantine: {config.policies.quarantine}")
    output.info(f"  rate_limit: {config.policies.rate_limit}")
    output.info(f"  path: {config.policies.path}")
    output.blank()
    output.step("Loop:")
    output.info(f"  max_iterations: {config.loop.max_iterations}")
    output.info(f"  timeout_seconds: {config.loop.timeout_seconds}")
    output.info(f"  auto_commit: {config.loop.auto_commit}")

    return 0


def cmd_distros(args: Namespace) -> int:
    """List available configuration distros."""
    from moss_cli.config import get_distro, list_distros

    output = setup_output(args)
    distros = list_distros()

    if getattr(args, "json", False):
        result = []
        for name in sorted(distros):
            distro = get_distro(name)
            if distro:
                result.append({"name": name, "description": distro.description})
        output_result(result, args)
        return 0

    output.header("Available Distros")

    for name in sorted(distros):
        distro = get_distro(name)
        if distro:
            desc = distro.description or "(no description)"
            output.info(f"  {name}: {desc}")

    return 0


# =============================================================================
# Codebase Tree Commands - delegated to Rust via passthrough in main()
# Python implementations removed, see git history for reference.
# =============================================================================


def cmd_context(args: Namespace) -> int:
    """Generate compiled context for a file (skeleton + deps + summary)."""
    from moss_intelligence.dependencies import extract_dependencies, format_dependencies
    from moss_intelligence.rust_shim import rust_available, rust_context
    from moss_intelligence.skeleton import extract_python_skeleton, format_skeleton

    output = setup_output(args)
    path = Path(args.path).resolve()

    if not path.exists():
        output.error(f"Path {path} does not exist")
        return 1

    if not path.is_file():
        output.error(f"{path} must be a file")
        return 1

    # Try Rust CLI for speed (10-100x faster)
    if rust_available():
        result = rust_context(str(path), root=str(path.parent))
        if result:
            if getattr(args, "json", False):
                output_result(result, args)
            else:
                # Format text output from Rust result
                summary = result.get("summary", {})
                output.header(path.name)
                output.info(f"Lines: {summary.get('lines', 0)}")
                output.info(
                    f"Classes: {summary.get('classes', 0)}, "
                    f"Functions: {summary.get('functions', 0)}, "
                    f"Methods: {summary.get('methods', 0)}"
                )
                output.info(
                    f"Imports: {summary.get('imports', 0)}, Exports: {summary.get('exports', 0)}"
                )
                output.blank()

                # Print imports
                imports = result.get("imports", [])
                if imports:
                    output.step("Imports")
                    for imp in imports:
                        module = imp.get("module", "")
                        names = imp.get("names", [])
                        if names:
                            output.print(f"from {module} import {', '.join(names)}")
                        else:
                            output.print(f"import {module}")
                    output.blank()

                # Print skeleton
                output.step("Skeleton")
                symbols = result.get("symbols", [])
                if symbols:
                    for sym in symbols:
                        sig = sym.get("signature", sym.get("name", ""))
                        output.print(sig)
                else:
                    output.verbose("(no symbols)")
            return 0

    # Read source file
    source = path.read_text()

    # Get skeleton and dependencies
    try:
        symbols = extract_python_skeleton(source)
        skeleton_content = format_skeleton(symbols)
    except Exception as e:
        output.error(f"Failed to extract skeleton: {e}")
        return 1

    try:
        deps_info = extract_dependencies(source)
        deps_content = format_dependencies(deps_info)
    except Exception as e:
        output.error(f"Failed to extract dependencies: {e}")
        return 1

    # Count symbols recursively
    def count_symbols(syms: list) -> dict:
        counts = {"classes": 0, "functions": 0, "methods": 0}
        for s in syms:
            kind = s.kind
            if kind == "class":
                counts["classes"] += 1
            elif kind == "function":
                counts["functions"] += 1
            elif kind == "method":
                counts["methods"] += 1
            if s.children:
                child_counts = count_symbols(s.children)
                for k, v in child_counts.items():
                    counts[k] += v
        return counts

    counts = count_symbols(symbols)
    line_count = len(source.splitlines())

    if getattr(args, "json", False):
        result = {
            "file": str(path),
            "summary": {
                "lines": line_count,
                "classes": counts["classes"],
                "functions": counts["functions"],
                "methods": counts["methods"],
                "imports": len(deps_info.imports),
                "exports": len(deps_info.exports),
            },
            "symbols": [s.to_dict() for s in symbols],
            "imports": [
                {"module": imp.module, "names": imp.names, "line": imp.lineno}
                for imp in deps_info.imports
            ],
            "exports": [
                {"name": exp.name, "type": exp.export_type, "line": exp.lineno}
                for exp in deps_info.exports
            ],
        }
        output_result(result, args)
    else:
        output.header(path.name)
        output.info(f"Lines: {line_count}")
        output.info(
            f"Classes: {counts['classes']}, "
            f"Functions: {counts['functions']}, Methods: {counts['methods']}"
        )
        output.info(f"Imports: {len(deps_info.imports)}, Exports: {len(deps_info.exports)}")
        output.blank()

        if deps_info.imports and deps_content:
            output.step("Imports")
            # Extract just the imports section from deps content
            imports_section = deps_content.split("Exports:")[0].strip()
            output.print(imports_section)
            output.blank()

        output.step("Skeleton")
        if skeleton_content:
            output.print(skeleton_content)
        else:
            output.verbose("(no symbols)")

    return 0


def cmd_search(args: Namespace) -> int:
    """Semantic search across codebase."""
    from moss import MossAPI

    out = get_output()
    directory = Path(args.directory).resolve()
    if not directory.exists():
        out.error(f"Directory {directory} does not exist")
        return 1

    api = MossAPI.for_project(directory)

    async def run_search():
        # Index if requested
        if args.index:
            patterns = args.patterns.split(",") if args.patterns else None
            count = await api.rag.index(patterns=patterns, force=False)
            if not args.query:
                out.success(f"Indexed {count} chunks from {directory}")
                return None

        if not args.query:
            out.error("No query provided. Use --query or --index")
            return None

        # Search
        return await api.rag.search(
            args.query,
            limit=args.limit,
            mode=args.mode,
        )

    results = asyncio.run(run_search())

    if results is None:
        return 0 if args.index else 1

    if not results:
        out.warning("No results found.")
        return 0

    if getattr(args, "json", False):
        json_results = [r.to_dict() for r in results]
        output_result(json_results, args)
    else:
        out.success(f"Found {len(results)} results:")
        out.blank()
        for i, r in enumerate(results, 1):
            location = f"{r.file_path}:{r.line_start}"
            name = r.symbol_name or r.file_path
            kind = r.symbol_kind or "file"
            score = f"{r.score:.2f}"

            out.info(f"{i}. [{kind}] {name}")
            out.print(f"   Location: {location}")
            out.print(f"   Score: {score} ({r.match_type})")

            # Show snippet
            if r.snippet:
                snippet = r.snippet[:200]
                if len(r.snippet) > 200:
                    snippet += "..."
                snippet_lines = snippet.split("\n")[:3]
                for line in snippet_lines:
                    out.print(f"   | {line}")
            out.blank()

    return 0


def cmd_gen(args: Namespace) -> int:
    """Generate interface code from MossAPI introspection."""
    import json as json_mod

    output = setup_output(args)
    target = getattr(args, "target", "mcp")
    out_file = getattr(args, "output", None)
    show_list = getattr(args, "list", False)

    try:
        if target == "mcp":
            from moss_orchestration.gen.mcp import MCPGenerator

            generator = MCPGenerator()
            if show_list:
                tools = generator.generate_tools()
                result = [
                    {"name": t.name, "description": t.description, "api_path": t.api_path}
                    for t in tools
                ]
                output.data(result)
            else:
                definitions = generator.generate_tool_definitions()
                content = json_mod.dumps(definitions, indent=2)
                if out_file:
                    Path(out_file).write_text(content)
                    output.success(
                        f"Generated {len(definitions)} MCP tool definitions to {out_file}"
                    )
                else:
                    print(content)

        elif target == "http":
            from moss_orchestration.gen.http import HTTPGenerator

            generator = HTTPGenerator()
            if show_list:
                routers = generator.generate_routers()
                result = []
                for router in routers:
                    for endpoint in router.endpoints:
                        result.append(
                            {
                                "path": endpoint.path,
                                "method": endpoint.method,
                                "description": endpoint.description,
                            }
                        )
                output.data(result)
            else:
                spec = generator.generate_openapi_spec()
                content = json_mod.dumps(spec, indent=2)
                if out_file:
                    Path(out_file).write_text(content)
                    output.success(f"Generated OpenAPI spec to {out_file}")
                else:
                    print(content)

        elif target == "cli":
            from moss_orchestration.gen.cli import CLIGenerator

            generator = CLIGenerator()
            if show_list:
                groups = generator.generate_groups()
                result = []
                for group in groups:
                    for cmd in group.commands:
                        result.append(
                            {
                                "command": f"{group.name} {cmd.name}",
                                "description": cmd.description,
                            }
                        )
                output.data(result)
            else:
                # Generate help text showing all commands
                parser = generator.generate_parser()
                parser.print_help()

        elif target == "openapi":
            from moss_orchestration.gen.http import HTTPGenerator

            generator = HTTPGenerator()
            spec = generator.generate_openapi_spec()
            content = json_mod.dumps(spec, indent=2)
            if out_file:
                Path(out_file).write_text(content)
                output.success(f"Generated OpenAPI spec to {out_file}")
            else:
                print(content)

        elif target == "grpc":
            from moss_orchestration.gen.grpc import GRPCGenerator

            generator = GRPCGenerator()
            if show_list:
                rpcs = generator.generate_rpcs()
                result = [
                    {
                        "name": rpc.name,
                        "request": rpc.request_type,
                        "response": rpc.response_type,
                    }
                    for rpc in rpcs
                ]
                output.data(result)
            else:
                content = generator.generate_proto()
                if out_file:
                    Path(out_file).write_text(content)
                    output.success(f"Generated proto file to {out_file}")
                else:
                    print(content)

        elif target == "lsp":
            from moss_orchestration.gen.lsp import LSPGenerator

            generator = LSPGenerator()
            if show_list:
                commands = generator.generate_commands()
                result = [
                    {
                        "command": cmd.command,
                        "title": cmd.title,
                        "description": cmd.description,
                    }
                    for cmd in commands
                ]
                output.data(result)
            else:
                # Output command list as JSON
                commands = generator.generate_command_list()
                content = json_mod.dumps(commands, indent=2)
                if out_file:
                    Path(out_file).write_text(content)
                    output.success(f"Generated {len(commands)} LSP commands to {out_file}")
                else:
                    print(content)

        else:
            output.error(f"Unknown target: {target}")
            return 1

        return 0
    except Exception as e:
        output.error(f"Generation failed: {e}")
        output.debug_traceback()
        return 1


def cmd_watch(args: Namespace) -> int:
    """Watch for file changes and re-run tests."""
    import asyncio
    import shlex

    from moss_orchestration.watch_tests import WatchRunner, WatchTestConfig

    output = setup_output(args)
    directory = Path(getattr(args, "directory", ".")).resolve()

    # Parse test command
    test_command = None
    cmd_str = getattr(args, "command", None)
    if cmd_str:
        test_command = shlex.split(cmd_str)

    # Build config
    config = WatchTestConfig(
        debounce_ms=getattr(args, "debounce", 500),
        clear_screen=not getattr(args, "no_clear", False),
        run_on_start=not getattr(args, "no_initial", False),
        incremental=getattr(args, "incremental", False),
    )
    if test_command:
        config.test_command = test_command

    watcher = WatchRunner(directory, config, output)

    try:
        asyncio.run(watcher.run())
        return 0
    except KeyboardInterrupt:
        return 0


def cmd_hooks(args: Namespace) -> int:
    """Manage git pre-commit hooks."""
    from moss_orchestration.hooks import (
        check_hooks_installed,
        generate_hook_config_yaml,
        install_hooks,
        uninstall_hooks,
    )

    output = setup_output(args)
    project_dir = Path(getattr(args, "directory", ".")).resolve()
    action = getattr(args, "action", "status")

    if action == "install":
        try:
            force = getattr(args, "force", False)
            install_hooks(project_dir, force=force)
            output.success("Pre-commit hooks installed successfully")
            return 0
        except FileNotFoundError as e:
            output.error(str(e))
            return 1
        except FileExistsError as e:
            output.error(str(e))
            return 1

    elif action == "uninstall":
        if uninstall_hooks(project_dir):
            output.success("Pre-commit hooks uninstalled")
            return 0
        else:
            output.warning("No moss hooks found to uninstall")
            return 0

    elif action == "config":
        # Generate pre-commit config
        try:
            config_yaml = generate_hook_config_yaml()
            output.print(config_yaml)
            return 0
        except ImportError:
            output.error("PyYAML not installed. Install with: pip install pyyaml")
            return 1

    else:  # status
        if check_hooks_installed(project_dir):
            output.success("Moss pre-commit hooks are installed")
        else:
            output.info("Moss pre-commit hooks are not installed")
            output.info("Run 'moss hooks install' to install them")
        return 0


def cmd_rules(args: Namespace) -> int:
    """Check code against custom rules."""
    from moss_orchestration.rules_single import (
        EngineConfig,
        Severity,
        create_engine_with_builtins,
        load_rules_from_config,
    )
    from moss_orchestration.sarif import SARIFConfig, generate_sarif, write_sarif

    output = setup_output(args)
    directory = Path(getattr(args, "directory", ".")).resolve()

    if not directory.exists():
        output.error(f"Directory {directory} does not exist")
        return 1

    # Load rules
    include_builtins = not getattr(args, "no_builtins", False)
    custom_rules = load_rules_from_config(directory)

    # Configure engine with file pattern
    pattern = getattr(args, "pattern", "**/*.py")
    config = EngineConfig(include_patterns=[pattern])

    engine = create_engine_with_builtins(
        include_builtins=include_builtins,
        custom_rules=custom_rules,
        config=config,
    )

    if not engine.rules:
        output.warning("No rules configured")
        return 0

    # List rules if requested
    if getattr(args, "list", False):
        output.header("Available Rules")
        for rule in engine.rules.values():
            status = "[enabled]" if rule.enabled else "[disabled]"
            backends = ", ".join(rule.backends)
            output.info(f"  {rule.name} ({backends}): {rule.description} {status}")
        return 0

    # Run analysis
    result = engine.check_directory(directory)

    if getattr(args, "json", False):
        output.data(result.to_dict())
        return 0

    # SARIF output
    sarif_path = getattr(args, "sarif", None)
    if sarif_path:
        from moss import __version__

        config = SARIFConfig(
            tool_name="moss",
            tool_version=__version__,
            base_path=directory,
        )
        sarif = generate_sarif(result, config)
        write_sarif(sarif, Path(sarif_path))
        output.success(f"SARIF output written to {sarif_path}")
        return 0

    # Text output
    if not result.violations:
        output.success(f"No violations found in {result.files_checked} files")
        return 0

    output.header(f"Found {len(result.violations)} violations")
    output.blank()

    # Group by file
    by_file: dict[Path, list] = {}
    for v in result.violations:
        file_path = v.location.file_path
        if file_path not in by_file:
            by_file[file_path] = []
        by_file[file_path].append(v)

    for file_path, violations in sorted(by_file.items()):
        try:
            rel_path = file_path.relative_to(directory)
        except ValueError:
            rel_path = file_path
        output.step(str(rel_path))

        for v in violations:
            severity_marker = {
                Severity.ERROR: "E",
                Severity.WARNING: "W",
                Severity.INFO: "I",
            }.get(v.severity, "?")
            output.info(f"  {v.location.line}:{v.location.column} [{severity_marker}] {v.message}")

        output.blank()

    # Summary
    errors = result.error_count
    warnings = result.warning_count
    infos = result.info_count
    output.info(f"Summary: {errors} errors, {warnings} warnings, {infos} info")

    # Return non-zero if errors found
    return 1 if errors > 0 else 0


def cmd_edit(args: Namespace) -> int:
    """Edit code with intelligent complexity routing."""
    from moss_intelligence.edit import EditContext, TaskComplexity, analyze_complexity, edit

    output = setup_output(args)
    project_dir = Path(getattr(args, "directory", ".")).resolve()
    task = args.task

    # Build context
    target_file = None
    if args.file:
        target_file = (project_dir / args.file).resolve()
        if not target_file.exists():
            output.error(f"File {target_file} does not exist")
            return 1

    context = EditContext(
        project_root=project_dir,
        target_file=target_file,
        target_symbol=getattr(args, "symbol", None),
        language=getattr(args, "language", "python"),
        constraints=args.constraint or [],
    )

    # Analyze complexity
    complexity = analyze_complexity(task, context)

    if getattr(args, "analyze_only", False):
        output.header("Complexity Analysis")
        output.info(f"Task: {task}")
        output.info(f"Complexity: {complexity.value}")

        # Show which patterns matched
        if complexity == TaskComplexity.SIMPLE:
            output.info("Handler: structural editing (refactoring)")
        elif complexity == TaskComplexity.MEDIUM:
            output.info("Handler: multi-agent decomposition")
        elif complexity == TaskComplexity.COMPLEX:
            output.info("Handler: synthesis (with multi-agent fallback)")
        else:
            output.info("Handler: synthesis (novel problem)")

        return 0

    # Show what we're doing
    output.step(f"Editing ({complexity.value} complexity)...")

    # Force specific handler if requested
    force_method = getattr(args, "method", None)
    if force_method:
        output.verbose(f"Forcing method: {force_method}")

    async def run_edit():
        if force_method == "structural":
            from moss_intelligence.edit import structural_edit

            return await structural_edit(task, context)
        elif force_method == "synthesis":
            from moss_intelligence.edit import synthesize_edit

            return await synthesize_edit(task, context)
        else:
            return await edit(task, context)

    try:
        result = asyncio.run(run_edit())
    except Exception as e:
        output.error(f"Edit failed: {e}")
        return 1

    # Output result
    if getattr(args, "json", False):
        output_result(
            {
                "success": result.success,
                "method": result.method,
                "changes": [
                    {
                        "file": str(c.path),
                        "has_changes": c.has_changes,
                        "description": c.description,
                    }
                    for c in result.changes
                ],
                "iterations": result.iterations,
                "error": result.error,
                "metadata": result.metadata,
            },
            args,
        )
    else:
        if result.success:
            output.success(f"Edit complete (method: {result.method})")

            if result.changes:
                output.blank()
                output.step(f"Changes ({len(result.changes)} files):")
                for change in result.changes:
                    if change.has_changes:
                        output.info(f"  {change.path}")
                        if change.description:
                            output.verbose(f"    {change.description}")

                # Show diff if requested
                if getattr(args, "diff", False):
                    output.blank()
                    output.step("Diff:")
                    for change in result.changes:
                        if change.has_changes:
                            import difflib

                            diff = difflib.unified_diff(
                                change.original.splitlines(keepends=True),
                                change.modified.splitlines(keepends=True),
                                fromfile=f"a/{change.path.name}",
                                tofile=f"b/{change.path.name}",
                            )
                            output.print("".join(diff))

                # Apply changes if not dry-run
                if not getattr(args, "dry_run", False):
                    for change in result.changes:
                        if change.has_changes:
                            change.path.parent.mkdir(parents=True, exist_ok=True)
                            change.path.write_text(change.modified)
                    output.success("Changes applied")
                else:
                    output.info("(dry-run mode, changes not applied)")
            else:
                output.info("No changes needed")
        else:
            output.error(f"Edit failed: {result.error}")
            return 1

    return 0


def cmd_pr(args: Namespace) -> int:
    """Generate PR review summary."""
    from moss_orchestration.pr_review import analyze_pr

    output = setup_output(args)
    repo_path = Path(getattr(args, "directory", ".")).resolve()

    try:
        review = analyze_pr(
            repo_path,
            from_ref=getattr(args, "base", "main"),
            to_ref=getattr(args, "head", "HEAD"),
            staged=getattr(args, "staged", False),
        )
    except Exception as e:
        output.error(f"Failed to analyze: {e}")
        return 1

    if review.diff_analysis.files_changed == 0:
        output.info("No changes found")
        return 0

    if getattr(args, "json", False):
        output.data(review.to_dict())
        return 0

    # Show title suggestion
    if getattr(args, "title", False):
        output.print(review.title_suggestion)
        return 0

    # Show full summary
    output.print(review.summary)

    return 0


def cmd_diff(args: Namespace) -> int:
    """Analyze git diff and show symbol changes."""
    from moss_orchestration.diff_analysis import (
        analyze_diff,
        get_commit_diff,
        get_staged_diff,
        get_working_diff,
    )

    output = setup_output(args)
    repo_path = Path(getattr(args, "directory", ".")).resolve()

    # Get the appropriate diff
    try:
        if getattr(args, "staged", False):
            diff_output = get_staged_diff(repo_path)
        elif getattr(args, "working", False):
            diff_output = get_working_diff(repo_path)
        else:
            from_ref = getattr(args, "from_ref", "HEAD~1")
            to_ref = getattr(args, "to_ref", "HEAD")
            diff_output = get_commit_diff(repo_path, from_ref, to_ref)
    except Exception as e:
        output.error(f"Failed to get diff: {e}")
        return 1

    if not diff_output.strip():
        output.info("No changes found")
        return 0

    # Analyze the diff
    analysis = analyze_diff(diff_output)

    if getattr(args, "json", False):
        output.data(analysis.to_dict())
        return 0

    # Show statistics summary only
    if getattr(args, "stat", False):
        output.info(f"Files: {analysis.files_changed} changed")
        if analysis.files_added:
            output.info(f"  {analysis.files_added} added")
        if analysis.files_deleted:
            output.info(f"  {analysis.files_deleted} deleted")
        if analysis.files_renamed:
            output.info(f"  {analysis.files_renamed} renamed")
        output.info(f"Lines: +{analysis.total_additions} -{analysis.total_deletions}")
        return 0

    # Full output
    output.print(analysis.summary)

    return 0


def cmd_check_refs(args: Namespace) -> int:
    """Check bidirectional references between code and docs."""
    from moss import MossAPI

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()

    if not root.exists():
        output.error(f"Directory not found: {root}")
        return 1

    staleness_days = getattr(args, "staleness_days", 30)
    api = MossAPI.for_project(root)

    output.info(f"Checking references in {root.name}...")

    try:
        result = api.ref_check.check(staleness_days=staleness_days)
    except Exception as e:
        output.error(f"Failed to check references: {e}")
        return 1

    # Output format
    compact = getattr(args, "compact", False)
    if compact and not wants_json(args):
        output.print(result.to_compact())
    elif wants_json(args):
        output.data(result.to_dict())
    else:
        output.print(result.to_markdown())

    # Exit codes
    if result.has_errors:
        return 1
    if getattr(args, "strict", False) and result.has_warnings:
        return 1

    return 0


def cmd_external_deps(args: Namespace) -> int:
    """Analyze external dependencies from pyproject.toml/requirements.txt."""
    from moss import MossAPI

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()

    if not root.exists():
        output.error(f"Directory not found: {root}")
        return 1

    api = MossAPI.for_project(root)
    resolve = getattr(args, "resolve", False)
    warn_weight = getattr(args, "warn_weight", 0)
    check_vulns = getattr(args, "check_vulns", False)
    check_licenses = getattr(args, "check_licenses", False)

    output.info(f"Analyzing dependencies in {root.name}...")

    try:
        result = api.external_deps.analyze(
            resolve=resolve, check_vulns=check_vulns, check_licenses=check_licenses
        )
    except Exception as e:
        output.error(f"Failed to analyze dependencies: {e}")
        return 1

    if not result.sources:
        output.warning("No dependency files found (pyproject.toml, requirements.txt)")
        return 0

    # Output format
    compact = getattr(args, "compact", False)
    if compact and not wants_json(args):
        output.print(result.to_compact())
    elif wants_json(args):
        output.data(result.to_dict(weight_threshold=warn_weight))
    else:
        output.print(result.to_markdown(weight_threshold=warn_weight))

    # Exit with error if heavy deps found and threshold set
    if warn_weight > 0 and result.get_heavy_dependencies(warn_weight):
        return 1

    # Exit with error if vulnerabilities found
    if check_vulns and result.has_vulnerabilities:
        return 1

    # Exit with error if license issues found
    if check_licenses and result.has_license_issues:
        return 1

    return 0


def cmd_roadmap(args: Namespace) -> int:
    """Show project roadmap and progress from TODO.md."""
    from moss_cli.roadmap import display_roadmap, find_todo_md

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()

    # Find TODO.md
    todo_path = find_todo_md(root)
    if todo_path is None:
        output.error("TODO.md not found")
        return 1

    # Determine display mode
    # --plain/--compact explicitly sets plain text (good for LLMs)
    # --tui explicitly sets TUI
    # Default: TUI if stdout is a TTY, plain otherwise
    use_tui = getattr(args, "tui", False)
    use_plain = getattr(args, "plain", False)
    use_compact = getattr(args, "compact", False)

    if use_plain or use_compact:
        tui = False
    elif use_tui:
        tui = True
    else:
        # Auto-detect: TUI for humans at terminal, plain for piping/LLMs
        import sys

        tui = sys.stdout.isatty()

    use_color = not getattr(args, "no_color", False) and tui
    width = getattr(args, "width", 80)
    show_completed = getattr(args, "completed", False)
    max_items = getattr(args, "max_items", 0)

    return display_roadmap(
        path=todo_path,
        tui=tui,
        show_completed=show_completed,
        use_color=use_color,
        width=width,
        max_items=max_items,
    )


def cmd_git_hotspots(args: Namespace) -> int:
    """Find frequently changed files in git history."""
    from moss import MossAPI

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()
    days = getattr(args, "days", 90)

    if not root.exists():
        output.error(f"Directory not found: {root}")
        return 1

    output.info(f"Analyzing git history for {root.name} (last {days} days)...")

    api = MossAPI.for_project(root)
    try:
        analysis = api.git_hotspots.analyze(days=days)
    except Exception as e:
        output.error(f"Failed to analyze git history: {e}")
        return 1

    if analysis.error:
        output.error(analysis.error)
        return 1

    # Output format
    compact = getattr(args, "compact", False)
    if compact and not wants_json(args):
        output.print(analysis.to_compact())
    elif wants_json(args):
        output.data(analysis.to_dict())
    else:
        output.print(analysis.to_markdown())

    return 0


def cmd_coverage(args: Namespace) -> int:
    """Show test coverage statistics."""
    from moss_intelligence.test_coverage import analyze_coverage

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()
    run_tests = getattr(args, "run", False)

    if not root.exists():
        output.error(f"Directory not found: {root}")
        return 1

    if run_tests:
        output.info(f"Running tests with coverage for {root.name}...")
    else:
        output.info(f"Checking coverage data for {root.name}...")

    try:
        report = analyze_coverage(root, run_tests=run_tests)
    except Exception as e:
        output.error(f"Failed to analyze coverage: {e}")
        return 1

    if report.error:
        output.error(report.error)
        return 1

    # Output format
    compact = getattr(args, "compact", False)
    if compact and not wants_json(args):
        output.print(report.to_compact())
    elif wants_json(args):
        output.data(report.to_dict())
    else:
        output.print(report.to_markdown())

    return 0


def cmd_lint(args: Namespace) -> int:
    """Run unified linting across multiple tools.

    Runs configured linters (ruff, mypy, etc.) and combines their output
    into a unified format.
    """
    import asyncio

    from moss_orchestration.plugins.linters import get_linter_registry

    output = setup_output(args)
    paths_arg = getattr(args, "paths", None) or ["."]
    paths = [Path(p).resolve() for p in paths_arg]

    # Validate paths
    for path in paths:
        if not path.exists():
            output.error(f"Path not found: {path}")
            return 1

    # Get registry and available linters
    registry = get_linter_registry()
    registry.register_builtins()

    # Filter by linter name if specified
    linter_names = getattr(args, "linters", None)
    if linter_names:
        linters = [registry.get(name) for name in linter_names.split(",")]
        linters = [linter for linter in linters if linter is not None]
        if not linters:
            output.error(f"No linters found matching: {linter_names}")
            output.info(
                f"Available: {', '.join(p.metadata.name for p in registry.get_available())}"
            )
            return 1
    else:
        linters = registry.get_available()

    if not linters:
        output.warning("No linters available")
        return 0

    linter_names_str = ", ".join(linter.metadata.name for linter in linters)
    output.info(f"Running {len(linters)} linter(s): {linter_names_str}")

    # Collect files to lint
    files_to_lint: list[Path] = []
    pattern = getattr(args, "pattern", "**/*.py")
    for path in paths:
        if path.is_file():
            files_to_lint.append(path)
        else:
            files_to_lint.extend(path.glob(pattern))

    if not files_to_lint:
        output.info("No files to lint")
        return 0

    output.info(f"Checking {len(files_to_lint)} file(s)...")

    # Run linters
    async def run_linters() -> list[tuple[str, Any]]:
        results = []
        for linter in linters:
            # Check file extension against supported languages
            supported_exts = {
                "python": {".py", ".pyi"},
                "javascript": {".js", ".jsx", ".mjs"},
                "typescript": {".ts", ".tsx"},
            }
            linter_exts: set[str] = set()
            for lang in linter.metadata.languages:
                linter_exts.update(supported_exts.get(lang, set()))

            for file_path in files_to_lint:
                if not linter_exts or file_path.suffix in linter_exts:
                    result = await linter.run(file_path)
                    results.append((linter.metadata.name, result))
        return results

    all_results = asyncio.run(run_linters())

    # Combine and format results
    total_issues = 0
    errors = 0
    warnings = 0
    grouped_by_file: dict[Path, list] = {}

    for linter_name, result in all_results:
        if not result.success:
            errors += 1
        for issue in result.issues:
            total_issues += 1
            if issue.severity.name == "ERROR":
                errors += 1
            elif issue.severity.name == "WARNING":
                warnings += 1

            file_key = issue.file or Path("unknown")
            if file_key not in grouped_by_file:
                grouped_by_file[file_key] = []
            grouped_by_file[file_key].append((linter_name, issue))

    # Output
    if wants_json(args):
        json_output = {
            "total_issues": total_issues,
            "errors": errors,
            "warnings": warnings,
            "files": {
                str(f): [
                    {
                        "linter": ln,
                        "message": i.message,
                        "severity": i.severity.name,
                        "line": i.line,
                        "column": i.column,
                        "rule_id": i.rule_id,
                    }
                    for ln, i in issues
                ]
                for f, issues in grouped_by_file.items()
            },
        }
        output.data(json_output)
    else:
        # Text output grouped by file
        for file_path, issues in sorted(grouped_by_file.items()):
            output.header(str(file_path))
            for _linter_name, issue in issues:
                loc = f":{issue.line}" if issue.line else ""
                loc += f":{issue.column}" if issue.column else ""
                rule = f" [{issue.rule_id}]" if issue.rule_id else ""
                severity = issue.severity.name.lower()
                output.print(f"  {loc} {severity}{rule}: {issue.message}")

        output.blank()
        if total_issues == 0:
            output.success("No issues found")
        else:
            output.info(f"Found {total_issues} issue(s): {errors} error(s), {warnings} warning(s)")

    # Return non-zero if errors found
    fix = getattr(args, "fix", False)
    if fix and errors == 0:
        output.info("Running fixes...")
        # TODO: Implement fix mode by calling linter.fix() methods
        output.warning("Fix mode not yet implemented")

    return 1 if errors > 0 else 0


def cmd_checkpoint(args: Namespace) -> int:
    """Manage checkpoints (shadow branches) for safe code modifications.

    Subcommands:
    - create: Create a checkpoint with current changes
    - list: List active checkpoints
    - diff: Show changes in a checkpoint
    - merge: Merge checkpoint changes into base branch
    - abort: Abandon a checkpoint
    - restore: Revert working directory to checkpoint state
    """
    import asyncio

    from moss import MossAPI

    output = setup_output(args)
    root = Path(".").resolve()
    action = getattr(args, "action", "list")
    name = getattr(args, "name", None)
    message = getattr(args, "message", None)

    # Verify we're in a git repo
    if not (root / ".git").exists():
        output.error("Not a git repository")
        return 1

    api = MossAPI.for_project(root)

    async def run_action() -> int:
        if action == "create":
            try:
                result = await api.git.create_checkpoint(name=name, message=message)
                output.success(f"Created checkpoint: {result['branch']}")
                output.info(f"Commit: {result['commit'][:8]}")
            except Exception as e:
                output.error(f"Failed to create checkpoint: {e}")
                return 1

        elif action == "list":
            try:
                checkpoints = await api.git.list_checkpoints()
                if not checkpoints:
                    output.info("No active checkpoints")
                else:
                    output.header("Active Checkpoints")
                    for cp in checkpoints:
                        output.print(f"    {cp['name']} ({cp['type']})")
            except Exception as e:
                output.error(f"Failed to list checkpoints: {e}")
                return 1

        elif action == "diff":
            if not name:
                output.error("Checkpoint name required for diff")
                return 1
            try:
                result = await api.git.diff_checkpoint(name)
                if result["diff"]:
                    output.print(result["diff"])
                else:
                    output.info("No differences")
            except Exception as e:
                output.error(f"Failed to get diff: {e}")
                return 1

        elif action == "merge":
            if not name:
                output.error("Checkpoint name required for merge")
                return 1
            try:
                result = await api.git.merge_checkpoint(name, message=message)
                output.success(f"Merged checkpoint {name}")
                output.info(f"Commit: {result['commit'][:8]}")
            except Exception as e:
                output.error(f"Failed to merge: {e}")
                return 1

        elif action == "abort":
            if not name:
                output.error("Checkpoint name required for abort")
                return 1
            try:
                await api.git.abort_checkpoint(name)
                output.success(f"Aborted checkpoint: {name}")
            except Exception as e:
                output.error(f"Failed to abort: {e}")
                return 1

        elif action == "restore":
            if not name:
                output.error("Checkpoint name required for restore")
                return 1
            try:
                result = await api.git.restore_checkpoint(name)
                output.success(f"Restored checkpoint: {name}")
                output.info(f"Now at commit: {result['commit'][:8]}")
            except Exception as e:
                output.error(f"Failed to restore: {e}")
                return 1

        else:
            output.error(f"Unknown action: {action}")
            return 1

        return 0

    return asyncio.run(run_action())


def cmd_security(args: Namespace) -> int:
    """Run security analysis with multiple tools."""
    from moss import MossAPI

    from moss_intelligence.security import format_security_analysis

    output = setup_output(args)
    root = Path(getattr(args, "directory", ".")).resolve()
    tools = getattr(args, "tools", None)
    min_severity = getattr(args, "severity", "low")

    if tools:
        tools = [t.strip() for t in tools.split(",")]

    if not root.exists():
        output.error(f"Directory not found: {root}")
        return 1

    output.info(f"Running security analysis on {root.name}...")

    api = MossAPI.for_project(root)
    try:
        analysis = api.security.analyze(tools=tools, min_severity=min_severity)
    except Exception as e:
        output.error(f"Security analysis failed: {e}")
        return 1

    if wants_json(args):
        output.data(analysis.to_dict())
    else:
        output.print(format_security_analysis(analysis))

    # Return non-zero if critical/high findings
    if analysis.critical_count > 0 or analysis.high_count > 0:
        return 1

    return 0


def cmd_workflow(args: Namespace) -> int:
    """Manage and run TOML-based workflows.

    Subcommands:
    - list: Show available workflows
    - show: Show workflow details
    - run: Execute a workflow on a file
    - generate: Auto-create workflows based on project
    - new: Scaffold a new workflow from template
    """
    import tomllib

    output = setup_output(args)
    action = getattr(args, "action", "list")
    project_root = Path(getattr(args, "directory", ".")).resolve()

    # Workflow directories
    builtin_dir = Path(__file__).parent.parent / "workflows"
    user_dir = project_root / ".moss" / "workflows"

    def find_workflow(name: str) -> Path | None:
        """Find workflow TOML by name."""
        for d in [user_dir, builtin_dir]:
            p = d / f"{name}.toml"
            if p.exists():
                return p
        return None

    if action == "list":
        output.header("Available Workflows")
        workflows: set[str] = set()
        for d in [builtin_dir, user_dir]:
            if d.exists():
                for p in d.glob("*.toml"):
                    workflows.add(p.stem)
        if not workflows:
            output.print("  (none found)")
        for name in sorted(workflows):
            try:
                path = find_workflow(name)
                if path:
                    with path.open("rb") as f:
                        data = tomllib.load(f)
                    desc = data.get("workflow", {}).get("description", "(no description)")
                    output.print(f"  {name}: {desc}")
            except Exception as e:
                output.print(f"  {name}: (error loading: {e})")
        return 0

    elif action == "new":
        from moss_orchestration.workflows.templates import TEMPLATES

        name = getattr(args, "workflow_name", None)
        if not name:
            # Interactive prompt if no name provided
            if sys.stdin.isatty():
                try:
                    name = input("Enter workflow name (e.g., validate-fix): ").strip()
                except (KeyboardInterrupt, EOFError):
                    print()
                    return 1

            if not name:
                output.error("Workflow name required")
                return 1

        template_name = getattr(args, "template", "agentic")
        template_content = TEMPLATES.get(template_name)
        if not template_content:
            output.error(f"Unknown template: {template_name}")
            return 1

        workflow_dir = project_root / ".moss" / "workflows"
        workflow_dir.mkdir(parents=True, exist_ok=True)

        file_path = workflow_dir / f"{name}.toml"
        if file_path.exists() and not getattr(args, "force", False):
            output.error(f"Workflow '{name}' already exists at {file_path}")
            output.info("Use --force to overwrite")
            return 1

        content = template_content.format(name=name)
        file_path.write_text(content)
        output.success(f"Created workflow '{name}' at {file_path}")

        output.blank()
        output.step("Next steps:")
        output.info(f"  1. Edit {file_path}")
        output.info(f"  2. Run with: moss workflow run {name} --file <file>")
        return 0

    elif action == "show":
        name = getattr(args, "workflow_name", None)
        if not name:
            output.error("Workflow name required")
            return 1

        workflow_path = find_workflow(name)
        if not workflow_path:
            output.error(f"Workflow not found: {name}")
            return 1

        try:
            with workflow_path.open("rb") as f:
                data = tomllib.load(f)
        except Exception as e:
            output.error(f"Failed to load workflow: {e}")
            return 1

        wf = data.get("workflow", {})

        if wants_json(args):
            output.data(data)
        else:
            output.header(f"Workflow: {wf.get('name', name)}")
            output.print(f"Description: {wf.get('description', '(none)')}")
            output.print(f"Version: {wf.get('version', '1.0')}")

            # Limits
            limits = wf.get("limits", {})
            output.print(f"Max turns: {limits.get('max_turns', 20)}")
            if timeout := limits.get("timeout_seconds"):
                output.print(f"Timeout: {timeout}s")

            # LLM config
            if llm := wf.get("llm"):
                output.print(f"LLM strategy: {llm.get('strategy', 'simple')}")
                if model := llm.get("model"):
                    output.print(f"Model: {model}")

            # Steps (for step-based workflows)
            if steps := data.get("steps"):
                output.print("")
                output.header("Steps")
                for i, step in enumerate(steps, 1):
                    step_name = step.get("name", "unnamed")
                    step_action = step.get("action", "?")
                    output.print(f"  {i}. {step_name} ({step_action})")
        return 0

    elif action == "run":
        name = getattr(args, "workflow_name", None)
        mock = getattr(args, "mock", False)
        verbose = getattr(args, "verbose", False)
        workflow_args = getattr(args, "workflow_args", None) or []

        if not name:
            output.error("Workflow name required")
            return 1

        # Find workflow TOML file
        # Check: .moss/workflows/{name}.toml, then src/moss/workflows/{name}.toml
        workflow_path = None
        search_paths = [
            project_root / ".moss" / "workflows" / f"{name}.toml",
            Path(__file__).parent.parent / "workflows" / f"{name}.toml",
        ]
        for p in search_paths:
            if p.exists():
                workflow_path = p
                break

        if not workflow_path:
            output.error(f"Workflow not found: {name}")
            output.info(f"Searched: {', '.join(str(p) for p in search_paths)}")
            return 1

        # Parse arguments
        extra_args: dict[str, str] = {}
        for arg in workflow_args:
            if "=" not in arg:
                output.error(f"Invalid argument format: {arg} (expected KEY=VALUE)")
                return 1
            key, value = arg.split("=", 1)
            extra_args[key] = value

        # Load and run using execution primitives
        from moss_orchestration.execution import (
            NoLLM,
            agent_loop,
            step_loop,
        )
        from moss_orchestration.execution import (
            load_workflow as load_exec_workflow,
        )

        config = load_exec_workflow(str(workflow_path))
        if mock and config.llm:
            config.llm = NoLLM(actions=["view README.md", "done"])

        # Get task from args (for agentic workflows)
        task = extra_args.pop("task", extra_args.pop("instruction", ""))

        output.info(f"Running workflow '{name}'" + (f": {task}" if task else ""))
        if verbose:
            output.info(f"Path: {workflow_path}")
            if config.context:
                output.info(f"Context: {type(config.context).__name__}")
            if config.llm:
                output.info(f"LLM: {type(config.llm).__name__}")
        output.info("")

        try:
            # Run directly with loaded config (supports mock)
            if config.steps:
                result = step_loop(
                    steps=config.steps,
                    context=config.context,
                    cache=config.cache,
                    retry=config.retry,
                    initial_context=extra_args if extra_args else None,
                )
            else:
                result = agent_loop(
                    task=task,
                    context=config.context,
                    cache=config.cache,
                    retry=config.retry,
                    llm=config.llm,
                    max_turns=config.max_turns,
                )
            output.success("\nCompleted!")
            if verbose:
                output.info(f"Final context:\n{result}")
            return 0
        except Exception as e:
            output.error(f"Workflow failed: {e}")
            if verbose:
                import traceback

                output.info(traceback.format_exc())
            return 1

    else:
        output.error(f"Unknown action: {action}")
        return 1


def cmd_agent(args: Namespace) -> int:
    """Run DWIM agent on a task. Alias for 'moss workflow run dwim'.

    Uses composable execution primitives with task tree context.
    """
    from moss_orchestration.execution import (
        NoLLM,
        agent_loop,
        load_workflow,
    )

    output = setup_output(args)
    task = getattr(args, "task", None)
    verbose = getattr(args, "verbose", False)
    mock = getattr(args, "mock", False)

    if not task:
        output.error("Usage: moss agent <task>")
        output.info('Example: moss agent "Fix the type error in Patch.apply"')
        return 1

    # Load dwim workflow
    dwim_toml = Path(__file__).parent.parent / "workflows" / "dwim.toml"
    if not dwim_toml.exists():
        output.error("dwim.toml workflow not found")
        return 1

    config = load_workflow(str(dwim_toml))
    if mock and config.llm:
        config.llm = NoLLM(actions=["view README.md", "done"])

    output.info(f"Starting agent: {task}")
    if verbose:
        output.info(f"Context: {type(config.context).__name__}")
        output.info(f"LLM: {type(config.llm).__name__}")
        output.info(f"Max turns: {config.max_turns}")
    output.info("")

    try:
        result = agent_loop(
            task=task,
            context=config.context,
            cache=config.cache,
            retry=config.retry,
            llm=config.llm,
            max_turns=config.max_turns,
        )
        output.success("\nCompleted!")
        if verbose:
            output.info(f"Final context:\n{result}")
        return 0
    except Exception as e:
        output.error(f"Agent failed: {e}")
        if verbose:
            import traceback

            output.info(traceback.format_exc())
        return 1


def cmd_help(args: Namespace) -> int:
    """Show detailed help for commands."""
    from moss_cli.help import (
        format_category_list,
        format_command_help,
        get_command_help,
    )

    output = setup_output(args)
    command = getattr(args, "topic", None)

    if not command:
        # Show categorized list
        output.print(format_category_list())
        return 0

    # Show help for specific command
    cmd = get_command_help(command)
    if not cmd:
        output.error(f"Unknown command: {command}")
        output.info("Run 'moss help' to see all commands.")
        return 1

    output.print(format_command_help(cmd))
    return 0


def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="moss",
        description="Headless agent orchestration layer for AI engineering",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {get_version()}")

    # Global output options
    parser.add_argument("--json", "-j", action="store_true", help="Output in JSON format")
    parser.add_argument(
        "--compact",
        "-c",
        action="store_true",
        help="Compact output (token-efficient for AI agents)",
    )
    parser.add_argument(
        "--jq",
        metavar="EXPR",
        help="Filter JSON output with jq expression (e.g., '.stats', '.dependencies[0]')",
    )
    parser.add_argument("--quiet", "-q", action="store_true", help="Quiet mode (errors only)")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    parser.add_argument("--debug", action="store_true", help="Debug output (most verbose)")
    parser.add_argument("--no-color", action="store_true", help="Disable colored output")

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # init command
    init_parser = subparsers.add_parser("init", help="Initialize a moss project")
    init_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Project directory (default: current)",
    )
    init_parser.add_argument(
        "--distro",
        "-d",
        help="Base distro to use (default: python)",
    )
    init_parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Overwrite existing config",
    )
    init_parser.set_defaults(func=cmd_init)

    # run command
    run_parser = subparsers.add_parser("run", help="Run a task")
    run_parser.add_argument("task", help="Task description")
    run_parser.add_argument(
        "--directory",
        "-C",
        default=".",
        help="Project directory (default: current)",
    )
    run_parser.add_argument(
        "--priority",
        "-p",
        default="normal",
        choices=["low", "normal", "high", "critical"],
        help="Task priority",
    )
    run_parser.add_argument(
        "--constraint",
        "-c",
        action="append",
        help="Add constraint (can be repeated)",
    )
    run_parser.add_argument(
        "--wait",
        "-w",
        action="store_true",
        help="Wait for task completion",
    )
    run_parser.set_defaults(func=cmd_run)

    # status command
    status_parser = subparsers.add_parser("status", help="Show status")
    status_parser.add_argument(
        "--directory",
        "-C",
        default=".",
        help="Project directory (default: current)",
    )
    status_parser.set_defaults(func=cmd_status)

    # config command
    config_parser = subparsers.add_parser("config", help="Show/validate configuration")
    config_parser.add_argument(
        "--directory",
        "-C",
        default=".",
        help="Project directory (default: current)",
    )
    config_parser.add_argument(
        "--validate",
        action="store_true",
        help="Validate configuration",
    )
    config_parser.add_argument(
        "--list-distros",
        action="store_true",
        help="List available distros",
    )
    config_parser.set_defaults(func=cmd_config)

    # distros command
    distros_parser = subparsers.add_parser("distros", help="List available distros")
    distros_parser.set_defaults(func=cmd_distros)

    # ==========================================================================
    # Introspection commands
    # Passthrough commands (tree, path, view, search-tree, expand, callers,
    # callees, skeleton, anchors) are handled directly in main() before
    # argparse. See RUST_PASSTHROUGH in main() and use `moss <cmd> --help`
    # for their usage.
    # ==========================================================================

    # context command
    context_parser = subparsers.add_parser(
        "context", help="Generate compiled context (skeleton + deps + summary)"
    )
    context_parser.add_argument("path", help="Python file to analyze")
    context_parser.set_defaults(func=cmd_context)

    # search command
    search_parser = subparsers.add_parser("search", help="Semantic search across codebase")
    search_parser.add_argument("--query", "-q", help="Search query (natural language or code)")
    search_parser.add_argument(
        "--directory", "-C", default=".", help="Directory to search (default: .)"
    )
    search_parser.add_argument(
        "--index", "-i", action="store_true", help="Index files before searching"
    )
    search_parser.add_argument(
        "--persist", "-p", action="store_true", help="Persist index to disk (uses ChromaDB)"
    )
    search_parser.add_argument("--patterns", help="Glob patterns to include (comma-separated)")
    search_parser.add_argument("--exclude", help="Glob patterns to exclude (comma-separated)")
    search_parser.add_argument(
        "--limit", "-n", type=int, default=10, help="Max results (default: 10)"
    )
    search_parser.add_argument(
        "--mode",
        choices=["hybrid", "tfidf", "embedding"],
        default="hybrid",
        help="Search mode (default: hybrid)",
    )
    search_parser.set_defaults(func=cmd_search)

    # gen command
    gen_parser = subparsers.add_parser(
        "gen", help="Generate interface code from MossAPI introspection"
    )
    gen_parser.add_argument(
        "--target",
        "-t",
        default="mcp",
        choices=["mcp", "http", "cli", "openapi", "grpc", "lsp"],
        help="Generation target (default: mcp)",
    )
    gen_parser.add_argument(
        "--output",
        "-o",
        metavar="FILE",
        help="Output file (default: stdout)",
    )
    gen_parser.add_argument(
        "--list",
        "-l",
        action="store_true",
        help="List generated items instead of full output",
    )
    gen_parser.set_defaults(func=cmd_gen)

    # watch command
    watch_parser = subparsers.add_parser("watch", help="Watch files and re-run tests on changes")
    watch_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to watch (default: current)",
    )
    watch_parser.add_argument(
        "-c",
        "--command",
        help="Custom test command (default: pytest -v)",
    )
    watch_parser.add_argument(
        "--debounce",
        type=int,
        default=500,
        help="Debounce delay in milliseconds (default: 500)",
    )
    watch_parser.add_argument(
        "--no-clear",
        action="store_true",
        help="Don't clear screen between runs",
    )
    watch_parser.add_argument(
        "--no-initial",
        action="store_true",
        help="Don't run tests on start",
    )
    watch_parser.add_argument(
        "--incremental",
        "-i",
        action="store_true",
        help="Only run tests related to changed files",
    )
    watch_parser.set_defaults(func=cmd_watch)

    # hooks command
    hooks_parser = subparsers.add_parser("hooks", help="Manage git pre-commit hooks")
    hooks_parser.add_argument(
        "action",
        nargs="?",
        choices=["install", "uninstall", "status", "config"],
        default="status",
        help="Action to perform (default: status)",
    )
    hooks_parser.add_argument(
        "-C",
        "--directory",
        default=".",
        help="Project directory (default: current)",
    )
    hooks_parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Force overwrite existing hooks",
    )
    hooks_parser.set_defaults(func=cmd_hooks)

    # diff command
    diff_parser = subparsers.add_parser("diff", help="Analyze git diff and show symbol changes")
    diff_parser.add_argument(
        "from_ref",
        nargs="?",
        default="HEAD~1",
        help="Starting commit reference (default: HEAD~1)",
    )
    diff_parser.add_argument(
        "to_ref",
        nargs="?",
        default="HEAD",
        help="Ending commit reference (default: HEAD)",
    )
    diff_parser.add_argument(
        "-C",
        "--directory",
        default=".",
        help="Repository directory (default: current)",
    )
    diff_parser.add_argument(
        "--staged",
        action="store_true",
        help="Analyze staged changes instead of commits",
    )
    diff_parser.add_argument(
        "--working",
        action="store_true",
        help="Analyze working directory changes (unstaged)",
    )
    diff_parser.add_argument(
        "--stat",
        action="store_true",
        help="Show only statistics summary",
    )
    diff_parser.set_defaults(func=cmd_diff)

    # pr command
    pr_parser = subparsers.add_parser("pr", help="Generate PR review summary")
    pr_parser.add_argument(
        "--base",
        "-b",
        default="main",
        help="Base branch to compare against (default: main)",
    )
    pr_parser.add_argument(
        "--head",
        default="HEAD",
        help="Head commit/branch (default: HEAD)",
    )
    pr_parser.add_argument(
        "-C",
        "--directory",
        default=".",
        help="Repository directory (default: current)",
    )
    pr_parser.add_argument(
        "--staged",
        action="store_true",
        help="Analyze staged changes instead",
    )
    pr_parser.add_argument(
        "--title",
        "-t",
        action="store_true",
        help="Only output suggested PR title",
    )
    pr_parser.set_defaults(func=cmd_pr)

    # rules command
    rules_parser = subparsers.add_parser("rules", help="Check code against custom analysis rules")
    rules_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to analyze (default: current)",
    )
    rules_parser.add_argument(
        "--pattern",
        "-p",
        default="**/*.py",
        help="Glob pattern for files (default: **/*.py)",
    )
    rules_parser.add_argument(
        "--list",
        "-l",
        action="store_true",
        help="List available rules",
    )
    rules_parser.add_argument(
        "--no-builtins",
        action="store_true",
        help="Disable built-in rules",
    )
    rules_parser.add_argument(
        "--sarif",
        "-s",
        help="Output results in SARIF format to file",
    )
    rules_parser.set_defaults(func=cmd_rules)

    # edit command
    edit_parser = subparsers.add_parser(
        "edit", help="Edit code with intelligent complexity routing"
    )
    edit_parser.add_argument(
        "task",
        help="Description of the edit task",
    )
    edit_parser.add_argument(
        "-f",
        "--file",
        help="Target file to edit",
    )
    edit_parser.add_argument(
        "-s",
        "--symbol",
        help="Target symbol (function, class, method) to edit",
    )
    edit_parser.add_argument(
        "-C",
        "--directory",
        default=".",
        help="Project directory (default: current)",
    )
    edit_parser.add_argument(
        "-l",
        "--language",
        default="python",
        help="Programming language (default: python)",
    )
    edit_parser.add_argument(
        "-c",
        "--constraint",
        action="append",
        help="Add constraint (can be repeated)",
    )
    edit_parser.add_argument(
        "--method",
        choices=["structural", "synthesis", "auto"],
        default="auto",
        help="Force specific edit method (default: auto)",
    )
    edit_parser.add_argument(
        "--analyze-only",
        "-a",
        action="store_true",
        dest="analyze_only",
        help="Only analyze complexity, don't edit",
    )
    edit_parser.add_argument(
        "--dry-run",
        action="store_true",
        dest="dry_run",
        help="Show what would change without applying",
    )
    edit_parser.add_argument(
        "--diff",
        "-d",
        action="store_true",
        help="Show unified diff of changes",
    )
    edit_parser.set_defaults(func=cmd_edit)

    # check-refs command
    check_refs_parser = subparsers.add_parser(
        "check-refs", help="Check bidirectional references between code and docs"
    )
    check_refs_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to check (default: current)",
    )
    check_refs_parser.add_argument(
        "--staleness-days",
        type=int,
        default=30,
        metavar="N",
        help="Warn if code changed more than N days after docs (default: 30)",
    )
    check_refs_parser.add_argument(
        "--strict",
        "-s",
        action="store_true",
        help="Exit with error on warnings (stale refs)",
    )
    check_refs_parser.add_argument(
        "--json",
        "-j",
        action="store_true",
        help="Output as JSON",
    )
    check_refs_parser.set_defaults(func=cmd_check_refs)

    # external-deps command
    external_deps_parser = subparsers.add_parser(
        "external-deps", help="Analyze external dependencies (PyPI packages)"
    )
    external_deps_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to analyze (default: current)",
    )
    external_deps_parser.add_argument(
        "--resolve",
        "-r",
        action="store_true",
        help="Resolve transitive dependencies (requires pip)",
    )
    external_deps_parser.add_argument(
        "--json",
        "-j",
        action="store_true",
        help="Output as JSON",
    )
    external_deps_parser.add_argument(
        "--warn-weight",
        "-w",
        type=int,
        default=0,
        metavar="N",
        help="Warn and exit 1 if any dependency has weight >= N (requires --resolve)",
    )
    external_deps_parser.add_argument(
        "--check-vulns",
        "-v",
        action="store_true",
        help="Check for known vulnerabilities via OSV API (exit 1 if found)",
    )
    external_deps_parser.add_argument(
        "--check-licenses",
        "-l",
        action="store_true",
        help="Check license compatibility (exit 1 if issues found)",
    )
    external_deps_parser.set_defaults(func=cmd_external_deps)

    # roadmap command
    roadmap_parser = subparsers.add_parser(
        "roadmap", help="Show project roadmap and progress from TODO.md"
    )
    roadmap_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to search for TODO.md (default: current)",
    )
    roadmap_parser.add_argument(
        "--tui",
        "-t",
        action="store_true",
        help="Use TUI display with box drawing (default for humans)",
    )
    roadmap_parser.add_argument(
        "--plain",
        "-p",
        action="store_true",
        help="Use plain text display (better for LLMs)",
    )
    roadmap_parser.add_argument(
        "--completed",
        "-c",
        action="store_true",
        help="Include completed phases",
    )
    roadmap_parser.add_argument(
        "--width",
        "-w",
        type=int,
        default=80,
        help="Terminal width for TUI mode (default: 80)",
    )
    roadmap_parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable colors in output",
    )
    roadmap_parser.add_argument(
        "--max-items",
        "-m",
        type=int,
        default=0,
        help="Max items per section (0 = unlimited, default: 0)",
    )
    roadmap_parser.add_argument(
        "--compact",
        action="store_true",
        help="Compact output (same as --plain)",
    )
    roadmap_parser.set_defaults(func=cmd_roadmap)

    # git-hotspots command
    hotspots_parser = subparsers.add_parser(
        "git-hotspots", help="Find frequently changed files in git history"
    )
    hotspots_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to analyze (default: current)",
    )
    hotspots_parser.add_argument(
        "--days",
        "-d",
        type=int,
        default=90,
        help="Number of days to analyze (default: 90)",
    )
    hotspots_parser.set_defaults(func=cmd_git_hotspots)

    # coverage command
    coverage_parser = subparsers.add_parser("coverage", help="Show test coverage statistics")
    coverage_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to analyze (default: current)",
    )
    coverage_parser.add_argument(
        "--run",
        "-r",
        action="store_true",
        help="Run pytest with coverage first",
    )
    coverage_parser.set_defaults(func=cmd_coverage)

    # lint command
    lint_parser = subparsers.add_parser("lint", help="Run unified linting across multiple tools")
    lint_parser.add_argument(
        "paths",
        nargs="*",
        default=["."],
        help="Paths to lint (default: current directory)",
    )
    lint_parser.add_argument(
        "--pattern",
        "-p",
        default="**/*.py",
        help="Glob pattern for files (default: **/*.py)",
    )
    lint_parser.add_argument(
        "--linters",
        "-l",
        help="Comma-separated list of linters to run (default: all available)",
    )
    lint_parser.add_argument(
        "--fix",
        "-f",
        action="store_true",
        help="Attempt to fix issues automatically",
    )
    lint_parser.set_defaults(func=cmd_lint)

    # checkpoint command
    checkpoint_parser = subparsers.add_parser(
        "checkpoint", help="Manage checkpoints (shadow branches) for safe code modifications"
    )
    checkpoint_parser.add_argument(
        "action",
        nargs="?",
        default="list",
        choices=["create", "list", "diff", "merge", "abort", "restore"],
        help="Action to perform (default: list)",
    )
    checkpoint_parser.add_argument(
        "name",
        nargs="?",
        help="Checkpoint name (required for diff, merge, abort)",
    )
    checkpoint_parser.add_argument(
        "--message",
        "-m",
        help="Message for create/merge operations",
    )
    checkpoint_parser.set_defaults(func=cmd_checkpoint)

    # workflow command
    workflow_parser = subparsers.add_parser("workflow", help="Manage and run TOML-based workflows")
    workflow_parser.add_argument(
        "action",
        nargs="?",
        default="list",
        choices=["list", "show", "run", "new"],
        help="Action: list, show, run, new (scaffold)",
    )
    workflow_parser.add_argument(
        "workflow_name",
        nargs="?",
        help="Workflow name (e.g., validate-fix)",
    )
    workflow_parser.add_argument(
        "--file",
        "-f",
        help="File to process (optional, defaults to codebase root)",
    )
    workflow_parser.add_argument(
        "--directory",
        "-C",
        default=".",
        help="Project directory for .moss/ lookup (default: current)",
    )
    workflow_parser.add_argument(
        "--mock",
        action="store_true",
        help="Use mock LLM responses (for testing)",
    )
    workflow_parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show LLM outputs and step details",
    )
    workflow_parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing workflows",
    )
    workflow_parser.add_argument(
        "--template",
        "-t",
        default="agentic",
        choices=["agentic", "step"],
        help="Template for new workflow (default: agentic)",
    )
    workflow_parser.add_argument(
        "--arg",
        "-a",
        action="append",
        dest="workflow_args",
        metavar="KEY=VALUE",
        help="Pass argument to workflow (repeatable, e.g., --arg model=gpt-4)",
    )
    workflow_parser.set_defaults(func=cmd_workflow)

    # security command
    security_parser = subparsers.add_parser(
        "security", help="Run security analysis with multiple tools"
    )
    security_parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Directory to analyze (default: current)",
    )
    security_parser.add_argument(
        "--tools",
        "-t",
        help="Comma-separated list of tools to use (default: all available)",
    )
    security_parser.add_argument(
        "--severity",
        "-s",
        choices=["low", "medium", "high", "critical"],
        default="low",
        help="Minimum severity to report (default: low)",
    )
    security_parser.set_defaults(func=cmd_security)

    # agent command - DWIM-driven agent loop
    agent_parser = subparsers.add_parser(
        "agent", help="Run DWIM agent (alias for 'workflow run dwim')"
    )
    agent_parser.add_argument(
        "task",
        nargs="?",
        help="Task description in natural language",
    )
    agent_parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show detailed output",
    )
    agent_parser.add_argument(
        "--mock",
        action="store_true",
        help="Use mock LLM responses (for testing)",
    )
    agent_parser.set_defaults(func=cmd_agent)

    # help command (with examples and categories)
    help_parser = subparsers.add_parser(
        "help", help="Show detailed help for commands with examples"
    )
    help_parser.add_argument(
        "topic",
        nargs="?",
        help="Command to get help for (omit for category list)",
    )
    help_parser.set_defaults(func=cmd_help)

    return parser


def _cmd_analyze_python(argv: list[str]) -> int:
    """Handle Python-only analyze flags (--summary, --check-docs, --check-todos)."""
    import argparse
    import json

    from moss import MossAPI

    parser = argparse.ArgumentParser(prog="moss analyze")
    parser.add_argument("target", nargs="?", default=".", help="Target path")
    parser.add_argument("--summary", action="store_true", help="Generate summary")
    parser.add_argument("--check-docs", action="store_true", help="Check documentation")
    parser.add_argument("--check-todos", action="store_true", help="Check TODOs")
    parser.add_argument("--json", action="store_true", help="JSON output")
    parser.add_argument("--compact", action="store_true", help="Compact output")
    parser.add_argument("--strict", action="store_true", help="Strict mode (exit 1 on warnings)")
    parser.add_argument("--check-links", action="store_true", help="Check doc links")
    parser.add_argument("--limit", type=int, default=10, help="Max items per section (default: 10)")
    parser.add_argument("--all", action="store_true", help="Show all items (override --limit)")
    parser.add_argument("--changed", action="store_true", help="Only check git-modified files")
    args = parser.parse_args(argv)

    root = Path(args.target).resolve()
    if not root.exists():
        print(f"Error: Path not found: {root}", file=sys.stderr)
        return 1

    api = MossAPI.for_project(root)

    if args.summary:
        from moss_intelligence.summarize import Summarizer

        summarizer = Summarizer()
        if root.is_file():
            result = summarizer.summarize_file(root)
        else:
            result = summarizer.summarize_project(root)

        if result is None:
            print(f"Error: Failed to summarize {root}", file=sys.stderr)
            return 1

        if args.json:
            print(json.dumps(result.to_dict(), indent=2))
        elif args.compact:
            print(result.to_compact())
        else:
            print(result.to_markdown())
        return 0

    # Determine limit: None if --all, otherwise use --limit value
    limit = None if getattr(args, "all", False) else args.limit

    # Get git-changed files if --changed flag is set
    changed_files: set[str] | None = None
    if args.changed:
        import subprocess

        try:
            # Get modified files (staged + unstaged + untracked)
            result_git = subprocess.run(
                ["git", "status", "--porcelain"],
                capture_output=True,
                text=True,
                cwd=root,
            )
            if result_git.returncode == 0:
                changed_files = set()
                for line in result_git.stdout.splitlines():
                    if len(line) > 3:
                        # Extract path from git status output (format: "XY filename")
                        path = line[3:].strip()
                        # Handle renamed files
                        if " -> " in path:
                            path = path.split(" -> ")[1]
                        changed_files.add(str(root / path))
        except (subprocess.SubprocessError, OSError):
            pass

    if args.check_docs:
        result = api.health.check_docs(check_links=args.check_links)
        # Filter to changed files if requested
        if changed_files is not None:
            result.issues = [i for i in result.issues if i.file and str(i.file) in changed_files]
        if args.json:
            print(json.dumps(result.to_dict(), indent=2))
        elif args.compact:
            print(result.to_compact())
        else:
            print(result.to_markdown(limit=limit))
        if result.has_errors:
            return 1
        if args.strict and result.has_warnings:
            return 1
        return 0

    if args.check_todos:
        result = api.health.check_todos()
        # Filter to changed files if requested
        if changed_files is not None:
            result.code_todos = [
                t for t in result.code_todos if str(root / t.source) in changed_files
            ]
        if args.json:
            print(json.dumps(result.to_dict(), indent=2))
        elif args.compact:
            print(result.to_compact())
        else:
            print(result.to_markdown(limit=limit))
        if args.strict and result.orphan_count > 0:
            return 1
        return 0

    # Fallback to Rust for other flags
    from moss_intelligence.rust_shim import passthrough

    return passthrough("analyze", argv)


def main(argv: list[str] | None = None) -> int:
    """Main entry point."""
    if argv is None:
        argv = sys.argv[1:]

    # Commands that delegate entirely to Rust CLI
    RUST_PASSTHROUGH = {"view", "analyze", "package", "grep", "sessions", "index", "daemon", "update"}

    # Python-only analyze flags (intercept before Rust passthrough)
    PYTHON_ANALYZE_FLAGS = {"--summary", "--check-docs", "--check-todos"}

    # Check for passthrough before argparse to avoid double-parsing
    if argv and argv[0] in RUST_PASSTHROUGH:
        # Intercept analyze with Python-only flags
        if argv[0] == "analyze" and any(f in argv for f in PYTHON_ANALYZE_FLAGS):
            return _cmd_analyze_python(argv[1:])

        from moss_intelligence.rust_shim import passthrough

        return passthrough(argv[0], argv[1:])

    parser = create_parser()
    args = parser.parse_args(argv)

    if not args.command:
        parser.print_help()
        return 0

    # Configure output based on global flags
    setup_output(args)

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
