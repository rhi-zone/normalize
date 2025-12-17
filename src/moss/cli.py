"""Command-line interface for Moss."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from argparse import Namespace


def get_version() -> str:
    """Get the moss version."""
    from moss import __version__

    return __version__


def cmd_init(args: Namespace) -> int:
    """Initialize a new moss project."""
    project_dir = Path(args.directory).resolve()

    if not project_dir.exists():
        print(f"Error: Directory {project_dir} does not exist")
        return 1

    config_file = project_dir / "moss_config.py"

    if config_file.exists() and not args.force:
        print(f"Error: {config_file} already exists. Use --force to overwrite.")
        return 1

    # Determine distro
    distro_name = args.distro or "python"

    config_content = f'''"""Moss configuration for this project."""

from pathlib import Path

from moss.config import MossConfig, get_distro

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
# config = config.with_static_context(Path("docs/architecture.md"))

# Add custom validators
# from moss.validators import CommandValidator
# config = config.add_validator(CommandValidator("mypy", ["mypy", "."]))
'''

    config_file.write_text(config_content)
    print(f"Created {config_file}")

    # Create .moss directory for runtime data
    moss_dir = project_dir / ".moss"
    if not moss_dir.exists():
        moss_dir.mkdir()
        (moss_dir / ".gitignore").write_text("*\n")
        print(f"Created {moss_dir}/")

    print(f"\nMoss initialized in {project_dir}")
    print(f"  Config: {config_file.name}")
    print(f"  Distro: {distro_name}")
    print("\nNext steps:")
    print("  1. Edit moss_config.py to customize your configuration")
    print("  2. Run 'moss run \"your task\"' to execute a task")

    return 0


def cmd_run(args: Namespace) -> int:
    """Run a task through moss."""
    from moss.agents import create_manager
    from moss.api import TaskRequest, create_api_handler
    from moss.config import MossConfig, load_config_file
    from moss.events import EventBus
    from moss.shadow_git import ShadowGit

    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    # Load config
    if config_file.exists():
        try:
            config = load_config_file(config_file)
        except Exception as e:
            print(f"Error loading config: {e}")
            return 1
    else:
        config = MossConfig().with_project(project_dir, project_dir.name)

    # Validate config
    errors = config.validate()
    if errors:
        print("Configuration errors:")
        for error in errors:
            print(f"  - {error}")
        return 1

    # Set up components
    event_bus = EventBus()
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
        print(f"Task created: {response.request_id}")
        print(f"Ticket: {response.ticket_id}")
        print(f"Status: {response.status.value}")

        if args.wait:
            print("\nWaiting for completion...")
            # Poll for status
            while True:
                status = await handler.get_task_status(response.request_id)
                if status is None:
                    print("Task not found")
                    return 1

                if status.status.value in ("COMPLETED", "FAILED", "CANCELLED"):
                    print(f"\nFinal status: {status.status.value}")
                    if status.result:
                        print(f"Result: {json.dumps(status.result, indent=2)}")
                    break

                await asyncio.sleep(0.5)

        return 0

    return asyncio.run(run_task())


def cmd_status(args: Namespace) -> int:
    """Show status of moss tasks and workers."""
    from moss.agents import create_manager
    from moss.api import create_api_handler
    from moss.config import load_config_file
    from moss.events import EventBus
    from moss.shadow_git import ShadowGit

    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    # Load config (validates it's readable)
    if config_file.exists():
        try:
            load_config_file(config_file)
        except Exception as e:
            print(f"Error loading config: {e}")
            return 1

    # Set up components
    event_bus = EventBus()
    shadow_git = ShadowGit(project_dir)
    manager = create_manager(shadow_git, event_bus)
    handler = create_api_handler(manager, event_bus)

    # Get stats
    stats = handler.get_stats()

    print("Moss Status")
    print("=" * 40)
    print(f"Project: {project_dir.name}")
    print(f"Config: {'moss_config.py' if config_file.exists() else '(default)'}")
    print()
    print("API Handler:")
    print(f"  Active requests: {stats['active_requests']}")
    print(f"  Pending checkpoints: {stats['pending_checkpoints']}")
    print(f"  Active streams: {stats['active_streams']}")
    print()
    print("Manager:")
    manager_stats = stats["manager_stats"]
    print(f"  Active workers: {manager_stats['active_workers']}")
    print(f"  Total tickets: {manager_stats['total_tickets']}")
    tickets_by_status = manager_stats.get("tickets_by_status", {})
    if tickets_by_status:
        print(f"  Tickets by status: {tickets_by_status}")

    if args.verbose:
        print()
        print("Workers:")
        for worker_id, worker_info in manager_stats.get("workers", {}).items():
            print(f"  {worker_id}: {worker_info}")

    return 0


def cmd_config(args: Namespace) -> int:
    """Show or validate configuration."""
    from moss.config import list_distros, load_config_file

    if args.list_distros:
        print("Available distros:")
        for name in list_distros():
            print(f"  - {name}")
        return 0

    project_dir = Path(args.directory).resolve()
    config_file = project_dir / "moss_config.py"

    if not config_file.exists():
        print(f"No config file found at {config_file}")
        print("Run 'moss init' to create one.")
        return 1

    try:
        config = load_config_file(config_file)
    except Exception as e:
        print(f"Error loading config: {e}")
        return 1

    if args.validate:
        errors = config.validate()
        if errors:
            print("Configuration errors:")
            for error in errors:
                print(f"  - {error}")
            return 1
        print("Configuration is valid.")
        return 0

    # Show config
    print("Configuration")
    print("=" * 40)
    print(f"Project: {config.project_name}")
    print(f"Root: {config.project_root}")
    print(f"Extends: {', '.join(config.extends) or '(none)'}")
    print()
    print("Validators:")
    print(f"  syntax: {config.validators.syntax}")
    print(f"  ruff: {config.validators.ruff}")
    print(f"  pytest: {config.validators.pytest}")
    print(f"  custom: {len(config.validators.custom)}")
    print()
    print("Policies:")
    print(f"  velocity: {config.policies.velocity}")
    print(f"  quarantine: {config.policies.quarantine}")
    print(f"  rate_limit: {config.policies.rate_limit}")
    print(f"  path: {config.policies.path}")
    print()
    print("Loop:")
    print(f"  max_iterations: {config.loop.max_iterations}")
    print(f"  timeout_seconds: {config.loop.timeout_seconds}")
    print(f"  auto_commit: {config.loop.auto_commit}")

    return 0


def cmd_distros(args: Namespace) -> int:
    """List available configuration distros."""
    from moss.config import get_distro, list_distros

    distros = list_distros()

    print("Available Distros")
    print("=" * 40)

    for name in sorted(distros):
        distro = get_distro(name)
        if distro:
            desc = distro.description or "(no description)"
            print(f"  {name}: {desc}")

    return 0


def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="moss",
        description="Headless agent orchestration layer for AI engineering",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {get_version()}")

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
    status_parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show verbose output",
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

    return parser


def main(argv: list[str] | None = None) -> int:
    """Main entry point."""
    parser = create_parser()
    args = parser.parse_args(argv)

    if not args.command:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
