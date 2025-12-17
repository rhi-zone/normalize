#!/usr/bin/env python3
"""Basic usage example for Moss.

This example demonstrates:
- Setting up core components (EventBus, ShadowGit, Manager)
- Creating and submitting tasks
- Checking task status
"""

import asyncio
from pathlib import Path

from moss import (
    EventBus,
    ShadowGit,
    TaskRequest,
    create_api_handler,
    create_manager,
)


async def main():
    # Initialize components
    project_dir = Path(".")  # Use current directory
    event_bus = EventBus()
    shadow_git = ShadowGit(project_dir)

    # Create manager and API handler
    manager = create_manager(shadow_git, event_bus)
    handler = create_api_handler(manager, event_bus)

    # Create a task request
    request = TaskRequest(
        task="Add a hello world function to main.py",
        priority="normal",
        constraints=["keep-existing-code"],
    )

    # Submit the task
    response = await handler.create_task(request)
    print("Task created!")
    print(f"  Request ID: {response.request_id}")
    print(f"  Ticket ID: {response.ticket_id}")
    print(f"  Status: {response.status.value}")

    # Check task status
    status = await handler.get_task_status(response.request_id)
    if status:
        print(f"\nTask status: {status.status.value}")

    # Get overall stats
    stats = handler.get_stats()
    print("\nSystem stats:")
    print(f"  Active requests: {stats['active_requests']}")
    print(f"  Active workers: {stats['manager_stats']['active_workers']}")


if __name__ == "__main__":
    asyncio.run(main())
