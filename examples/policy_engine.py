#!/usr/bin/env python3
"""Policy engine example for Moss.

This example demonstrates:
- Creating a policy engine
- Evaluating tool calls against policies
- Understanding policy decisions
- Custom policies
"""

import asyncio
import tempfile
from pathlib import Path

from moss import (
    PathPolicy,
    Policy,
    PolicyDecision,
    PolicyEngine,
    PolicyResult,
    QuarantinePolicy,
    RateLimitPolicy,
    ToolCallContext,
    VelocityPolicy,
    create_default_policy_engine,
)


async def main():
    print("=== Default Policy Engine ===")
    engine = create_default_policy_engine()
    print(f"Policies: {[p.name for p in engine.policies]}")

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)

        # Check a normal file edit
        print("\n--- Checking normal file edit ---")
        target = tmp_path / "src" / "main.py"
        result = await engine.check("edit", target=target)
        print(f"Allowed: {result.allowed}")
        print(f"Policies checked: {len(result.results)}")

        # Check a blocked path (.git)
        print("\n--- Checking .git path (should be blocked) ---")
        git_target = tmp_path / ".git" / "config"
        result = await engine.check("edit", target=git_target)
        print(f"Allowed: {result.allowed}")
        if result.blocking_result:
            print(f"Blocked by: {result.blocking_result.policy_name}")
            print(f"Reason: {result.blocking_result.reason}")

        # Check .env file
        print("\n--- Checking .env file (should be blocked) ---")
        env_target = tmp_path / ".env"
        result = await engine.check("edit", target=env_target)
        print(f"Allowed: {result.allowed}")
        if result.blocking_result:
            print(f"Blocked by: {result.blocking_result.policy_name}")

    print("\n=== Velocity Policy Demo ===")
    velocity = VelocityPolicy(stall_threshold=3, oscillation_threshold=2)

    # Simulate progress (decreasing errors)
    print("Simulating progress (errors going down)...")
    velocity.record_error_count(10)
    velocity.record_error_count(7)
    velocity.record_error_count(4)

    result = await velocity.evaluate(ToolCallContext(tool_name="edit"))
    print(f"After progress - Allowed: {result.allowed}")

    # Simulate stall (same error count)
    print("\nSimulating stall (errors staying the same)...")
    velocity.reset()
    for _ in range(4):
        velocity.record_error_count(5)

    result = await velocity.evaluate(ToolCallContext(tool_name="edit"))
    print(f"After stall - Allowed: {result.allowed}")
    if not result.allowed:
        print(f"Reason: {result.reason}")

    print("\n=== Quarantine Policy Demo ===")
    quarantine = QuarantinePolicy(repair_tools={"fix_syntax", "repair"})

    with tempfile.NamedTemporaryFile(suffix=".py", delete=False) as f:
        broken_file = Path(f.name)

    # Quarantine the file
    quarantine.quarantine(broken_file, "Syntax error at line 5")
    print(f"Quarantined files: {len(quarantine.quarantined_files)}")

    # Try to edit (should be blocked)
    result = await quarantine.evaluate(ToolCallContext(tool_name="edit", target=broken_file))
    print(f"Edit allowed: {result.allowed}")
    print(f"Decision: {result.decision.name}")

    # Try repair tool (should be allowed with warning)
    result = await quarantine.evaluate(ToolCallContext(tool_name="fix_syntax", target=broken_file))
    print(f"Repair allowed: {result.allowed}")
    print(f"Decision: {result.decision.name}")

    # Release from quarantine
    quarantine.release(broken_file)
    print(f"Released. Quarantined files: {len(quarantine.quarantined_files)}")

    broken_file.unlink()

    print("\n=== Rate Limit Policy Demo ===")
    rate_limit = RateLimitPolicy(max_calls_per_minute=3, max_calls_per_target=2)

    with tempfile.NamedTemporaryFile(suffix=".py", delete=False) as f:
        target_file = Path(f.name)

    # Make calls up to the limit
    for _ in range(3):
        rate_limit.record_call()

    result = await rate_limit.evaluate(ToolCallContext(tool_name="edit"))
    print(f"After {3} global calls - Allowed: {result.allowed}")
    if not result.allowed:
        print(f"Reason: {result.reason}")

    target_file.unlink()

    print("\n=== Custom Policy Example ===")

    class NoDeletePolicy(Policy):
        """Policy that blocks delete operations."""

        @property
        def name(self) -> str:
            return "no_delete"

        @property
        def priority(self) -> int:
            return 100  # High priority

        async def evaluate(self, context: ToolCallContext) -> PolicyResult:
            if context.tool_name == "delete":
                return PolicyResult(
                    decision=PolicyDecision.DENY,
                    policy_name=self.name,
                    reason="Delete operations are not allowed",
                )
            return PolicyResult(
                decision=PolicyDecision.ALLOW,
                policy_name=self.name,
            )

    # Create engine with custom policy
    custom_engine = PolicyEngine(policies=[NoDeletePolicy(), PathPolicy()])

    result = await custom_engine.check("edit")
    print(f"Edit allowed: {result.allowed}")

    result = await custom_engine.check("delete")
    print(f"Delete allowed: {result.allowed}")
    if not result.allowed:
        print(f"Reason: {result.blocking_result.reason}")


if __name__ == "__main__":
    asyncio.run(main())
