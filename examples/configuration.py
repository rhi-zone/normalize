#!/usr/bin/env python3
"""Configuration system example for Moss.

This example demonstrates:
- Creating configurations with the fluent API
- Using built-in distros
- Custom distro creation
- Configuration validation
"""

import tempfile
from pathlib import Path

from moss import (
    Distro,
    MossConfig,
    create_config,
    get_distro,
    list_distros,
    register_distro,
)


def main():
    print("=== Available Distros ===")
    for name in list_distros():
        distro = get_distro(name)
        if distro:
            print(f"  {name}: {distro.description or '(no description)'}")

    print("\n=== Creating config from 'python' distro ===")
    config = create_config("python")
    print(f"Project name: {config.project_name}")
    print(f"Extends: {config.extends}")
    print(f"Validators - syntax: {config.validators.syntax}, ruff: {config.validators.ruff}")

    print("\n=== Creating config with fluent API ===")
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)

        config = (
            MossConfig()
            .with_project(tmp_path, "my-awesome-project")
            .with_validators(syntax=True, ruff=True, pytest=True)
            .with_policies(velocity=True, quarantine=True, rate_limit=False)
            .with_loop(max_iterations=15, timeout_seconds=600, auto_commit=True)
        )

        print(f"Project name: {config.project_name}")
        print(f"Project root: {config.project_root}")
        print("Validators:")
        print(f"  - syntax: {config.validators.syntax}")
        print(f"  - ruff: {config.validators.ruff}")
        print(f"  - pytest: {config.validators.pytest}")
        print("Policies:")
        print(f"  - velocity: {config.policies.velocity}")
        print(f"  - quarantine: {config.policies.quarantine}")
        print(f"  - rate_limit: {config.policies.rate_limit}")
        print("Loop config:")
        print(f"  - max_iterations: {config.loop.max_iterations}")
        print(f"  - timeout_seconds: {config.loop.timeout_seconds}")
        print(f"  - auto_commit: {config.loop.auto_commit}")

        # Validate the configuration
        print("\n=== Validating configuration ===")
        errors = config.validate()
        if errors:
            print("Validation errors:")
            for error in errors:
                print(f"  - {error}")
        else:
            print("Configuration is valid!")

    print("\n=== Creating custom distro ===")
    # Create a custom distro that extends 'python'
    python_distro = get_distro("python")
    custom_distro = Distro(
        name="my-strict-python",
        description="Python with strict settings",
        extends=[python_distro] if python_distro else [],
    ).modify(lambda c: c.with_validators(pytest=True).with_loop(max_iterations=5))

    # Register it
    register_distro(custom_distro)

    # Use it
    config = create_config("my-strict-python")
    print("Custom distro config:")
    print(f"  Extends: {config.extends}")
    print(f"  pytest enabled: {config.validators.pytest}")
    print(f"  max_iterations: {config.loop.max_iterations}")

    print("\n=== Building validator chain from config ===")
    chain = config.build_validator_chain()
    print(f"Validators in chain: {[v.name for v in chain.validators]}")

    print("\n=== Building policies from config ===")
    policies = config.build_policies()
    print(f"Active policies: {[p.name for p in policies]}")


if __name__ == "__main__":
    main()
