#!/usr/bin/env python3
"""Validation chain example for Moss.

This example demonstrates:
- Creating validator chains
- Running validation on files
- Handling validation results
"""

import asyncio
import tempfile
from pathlib import Path

from moss import (
    SyntaxValidator,
    ValidationSeverity,
    ValidatorChain,
    create_python_validator_chain,
)


async def main():
    # Create a temporary Python file with an error
    with tempfile.NamedTemporaryFile(mode="w", suffix=".py", delete=False) as f:
        f.write("""
def hello():
    print("Hello, World!"
    return True
""")
        temp_path = Path(f.name)

    print("=== Using built-in Python validator chain ===")
    chain = create_python_validator_chain()
    print(f"Validators: {[v.name for v in chain.validators]}")

    result = await chain.validate(temp_path)
    print(f"\nValidation passed: {result.passed}")

    if not result.passed:
        print("\nIssues found:")
        for issue in result.issues:
            severity = "ERROR" if issue.severity == ValidationSeverity.ERROR else "WARN"
            print(f"  [{severity}] {issue.message}")
            if issue.file:
                print(f"    File: {issue.file}")
            if issue.line:
                print(f"    Line: {issue.line}")

    # Create a file that passes validation
    with tempfile.NamedTemporaryFile(mode="w", suffix=".py", delete=False) as f:
        f.write("""
def hello():
    print("Hello, World!")
    return True
""")
        good_path = Path(f.name)

    print("\n=== Validating correct file ===")
    result = await chain.validate(good_path)
    print(f"Validation passed: {result.passed}")

    # Custom validator chain
    print("\n=== Custom validator chain (syntax only) ===")
    custom_chain = ValidatorChain([SyntaxValidator()])
    result = await custom_chain.validate(good_path)
    print(f"Syntax check passed: {result.passed}")

    # Clean up
    temp_path.unlink()
    good_path.unlink()


if __name__ == "__main__":
    asyncio.run(main())
