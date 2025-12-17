#!/usr/bin/env python3
"""Event system example for Moss.

This example demonstrates:
- Creating an EventBus
- Subscribing to events
- Emitting events
- Event filtering
"""

import asyncio

from moss import Event, EventBus, EventType


async def main():
    # Create event bus
    bus = EventBus()
    print("Event bus created")

    # Track received events
    received_events = []

    # Subscribe to specific event types
    async def on_tool_call(event: Event):
        print(f"  Tool called: {event.data.get('tool', 'unknown')}")
        received_events.append(event)

    async def on_validation(event: Event):
        print(f"  Validation: {event.data.get('status', 'unknown')}")
        received_events.append(event)

    async def on_any_event(event: Event):
        print(f"  [Any] Event type: {event.event_type.name}")

    # Subscribe handlers
    bus.subscribe(EventType.TOOL_CALL, on_tool_call)
    bus.subscribe(EventType.VALIDATION_FAILED, on_validation)
    bus.subscribe(EventType.VALIDATION_PASSED, on_validation)

    # Subscribe to all events (catch-all)
    for event_type in EventType:
        bus.subscribe(event_type, on_any_event)

    print("\n=== Emitting events ===")

    # Emit tool call event
    print("\nEmitting TOOL_CALL:")
    await bus.emit(Event(EventType.TOOL_CALL, {"tool": "edit", "file": "main.py"}))

    # Emit validation events
    print("\nEmitting VALIDATION_PASSED:")
    await bus.emit(Event(EventType.VALIDATION_PASSED, {"status": "passed", "file": "main.py"}))

    print("\nEmitting VALIDATION_FAILED:")
    await bus.emit(
        Event(
            EventType.VALIDATION_FAILED,
            {"status": "failed", "errors": ["syntax error on line 5"]},
        )
    )

    # Emit shadow commit event
    print("\nEmitting SHADOW_COMMIT:")
    await bus.emit(Event(EventType.SHADOW_COMMIT, {"sha": "abc123", "message": "Add feature"}))

    print("\n=== Summary ===")
    print(f"Total events received by specific handlers: {len(received_events)}")

    # Unsubscribe
    bus.unsubscribe(EventType.TOOL_CALL, on_tool_call)
    print("\nUnsubscribed from TOOL_CALL")

    print("\nEmitting another TOOL_CALL (handler unsubscribed):")
    await bus.emit(Event(EventType.TOOL_CALL, {"tool": "delete", "file": "old.py"}))


if __name__ == "__main__":
    asyncio.run(main())
