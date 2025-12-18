"""Event Bus: Async pub/sub with typed events.

# See: docs/architecture/events.md
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Coroutine
from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import Enum, auto
from typing import Any
from uuid import UUID, uuid4


class EventType(Enum):
    """Core event types for the system."""

    USER_MESSAGE = auto()
    PLAN_GENERATED = auto()
    TOOL_CALL = auto()
    VALIDATION_FAILED = auto()
    SHADOW_COMMIT = auto()
    # Interrupt events
    INTERRUPT_CANCEL = auto()
    INTERRUPT_REDIRECT = auto()
    INTERRUPT_PAUSE = auto()


@dataclass(frozen=True, kw_only=True)
class Event:
    """Base event with metadata."""

    id: UUID = field(default_factory=uuid4)
    timestamp: datetime = field(default_factory=lambda: datetime.now(UTC))
    type: EventType
    payload: dict[str, Any] = field(default_factory=dict)


# Type alias for event handlers
EventHandler = Callable[[Event], Coroutine[Any, Any, None]]


class EventBus:
    """Async event bus with typed pub/sub."""

    def __init__(self) -> None:
        self._handlers: dict[EventType, list[EventHandler]] = {}
        self._all_handlers: list[EventHandler] = []
        self._history: list[Event] = []
        self._lock = asyncio.Lock()

    def subscribe(self, event_type: EventType, handler: EventHandler) -> Callable[[], None]:
        """Subscribe to a specific event type. Returns unsubscribe function."""
        if event_type not in self._handlers:
            self._handlers[event_type] = []
        self._handlers[event_type].append(handler)

        def unsubscribe() -> None:
            self._handlers[event_type].remove(handler)

        return unsubscribe

    def subscribe_all(self, handler: EventHandler) -> Callable[[], None]:
        """Subscribe to all events. Returns unsubscribe function."""
        self._all_handlers.append(handler)

        def unsubscribe() -> None:
            self._all_handlers.remove(handler)

        return unsubscribe

    async def publish(self, event: Event) -> None:
        """Publish an event to all subscribers."""
        async with self._lock:
            self._history.append(event)

        # Gather handlers for this event type
        handlers = list(self._all_handlers)
        if event.type in self._handlers:
            handlers.extend(self._handlers[event.type])

        # Run all handlers concurrently
        if handlers:
            await asyncio.gather(*[h(event) for h in handlers], return_exceptions=True)

    async def emit(
        self,
        event_type: EventType,
        payload: dict[str, Any] | None = None,
    ) -> Event:
        """Convenience method to create and publish an event."""
        event = Event(type=event_type, payload=payload or {})
        await self.publish(event)
        return event

    def history(self, event_type: EventType | None = None) -> list[Event]:
        """Get event history, optionally filtered by type."""
        if event_type is None:
            return list(self._history)
        return [e for e in self._history if e.type == event_type]

    def clear_history(self) -> None:
        """Clear event history."""
        self._history.clear()
