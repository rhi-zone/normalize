"""TaskTree: Hierarchical task state for context-excluded agent loops.

The task tree represents the agent's current work as a path from root goal
to current leaf action. Completed work collapses to summaries. Notes attach
with expiration conditions.

Design principles:
- Context-excluded: no conversation history, just path + notes
- Levels emerge from task, not predefined
- Recursive breakdown is fundamental
- ~300 tokens typical prompt size
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto
from pathlib import Path
from typing import Any


class TaskStatus(Enum):
    """Status of a task node."""

    PENDING = auto()  # Not started
    ACTIVE = auto()  # Currently working on
    DONE = auto()  # Completed successfully
    BLOCKED = auto()  # Cannot proceed


class NoteExpiry(Enum):
    """When a note should expire."""

    ON_DONE = auto()  # When current task completes
    MANUAL = auto()  # Only when explicitly removed
    ON_SUBTASK_DONE = auto()  # When any subtask completes


@dataclass
class Note:
    """Attached note with expiration condition.

    Notes travel with context and expire based on conditions.
    Use for: caller lists, constraints, temporary findings.
    """

    content: str
    expiry: NoteExpiry = NoteExpiry.ON_DONE
    scope: str | None = None  # Which subtree it applies to (None = current)
    turns_remaining: int | None = None  # For after:N expiry

    def should_expire(self, event: str) -> bool:
        """Check if note should expire given an event."""
        if self.expiry == NoteExpiry.MANUAL:
            return False
        if self.expiry == NoteExpiry.ON_DONE and event == "done":
            return True
        if self.expiry == NoteExpiry.ON_SUBTASK_DONE and event == "subtask_done":
            return True
        if self.turns_remaining is not None:
            return self.turns_remaining <= 0
        return False

    def tick(self) -> None:
        """Decrement turn counter if applicable."""
        if self.turns_remaining is not None:
            self.turns_remaining -= 1


@dataclass
class TaskNode:
    """A node in the task tree.

    Each node represents a goal at some level of decomposition.
    Can have children (subtasks) and a parent (enclosing task).
    """

    goal: str  # What this task aims to accomplish
    status: TaskStatus = TaskStatus.PENDING
    summary: str | None = None  # One-line result when done
    description: str | None = None  # Expandable detail (on demand)
    children: list[TaskNode] = field(default_factory=list)
    parent: TaskNode | None = field(default=None, repr=False)
    notes: list[Note] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)
    sandbox_scope: Path | None = None  # Restricted workspace for this task

    def add_child(self, goal: str, sandbox_scope: Path | None = None) -> TaskNode:
        """Add a subtask.

        Args:
            goal: The subtask goal
            sandbox_scope: Optional override for sandbox scope.
                         If None, inherits from parent.
        """
        # Inherit scope if not explicitly provided
        effective_scope = sandbox_scope if sandbox_scope is not None else self.sandbox_scope
        child = TaskNode(goal=goal, parent=self, sandbox_scope=effective_scope)
        self.children.append(child)
        return child

    def add_note(
        self,
        content: str,
        expiry: NoteExpiry = NoteExpiry.ON_DONE,
        turns: int | None = None,
    ) -> Note:
        """Attach a note to this node."""
        note = Note(
            content=content,
            expiry=expiry,
            turns_remaining=turns,
        )
        self.notes.append(note)
        return note

    def mark_done(self, summary: str) -> None:
        """Mark task as done with a summary."""
        self.status = TaskStatus.DONE
        self.summary = summary
        # Expire notes
        self.notes = [n for n in self.notes if not n.should_expire("done")]

    def mark_active(self) -> None:
        """Mark task as currently active."""
        self.status = TaskStatus.ACTIVE

    def mark_blocked(self, reason: str | None = None) -> None:
        """Mark task as blocked."""
        self.status = TaskStatus.BLOCKED
        if reason:
            self.add_note(f"Blocked: {reason}", NoteExpiry.MANUAL)

    @property
    def is_leaf(self) -> bool:
        """Check if this is a leaf node (no children)."""
        return len(self.children) == 0

    @property
    def active_child(self) -> TaskNode | None:
        """Get the currently active child, if any."""
        for child in self.children:
            if child.status == TaskStatus.ACTIVE:
                return child
        return None

    @property
    def path_to_root(self) -> list[TaskNode]:
        """Get path from this node to root (inclusive)."""
        path = [self]
        node = self
        while node.parent:
            path.append(node.parent)
            node = node.parent
        return list(reversed(path))

    def format_path(self, include_notes: bool = True) -> str:
        """Format the path to this node for prompt inclusion.

        Returns compact representation like:
        Task: Fix auth bug
          → Find failure point ✓ (token expires during refresh)
          → Implement fix
            → [now] Patching refresh_token()
        """
        lines = []
        path = self.path_to_root

        for i, node in enumerate(path):
            indent = "  " * i
            is_current = node is self

            if i == 0:
                # Root task
                prefix = "Task: "
            elif node.status == TaskStatus.DONE:
                prefix = "→ "
            elif is_current:
                prefix = "→ [now] "
            else:
                prefix = "→ "

            line = f"{indent}{prefix}{node.goal}"
            if node.status == TaskStatus.DONE and node.summary:
                line += f" ✓ ({node.summary})"
            elif is_current:
                pass  # Already marked with [now]

            # Add scope indicator if present
            if node.sandbox_scope:
                line += f" [scope: {node.sandbox_scope}]"

            lines.append(line)

        # Add notes if requested
        if include_notes:
            all_notes = self._collect_notes()
            for note in all_notes:
                lines.append(f"\n[note: {note.content}]")

        return "\n".join(lines)

    def _collect_notes(self) -> list[Note]:
        """Collect all active notes from path to root."""
        notes = []
        for node in self.path_to_root:
            notes.extend(node.notes)
        return notes


class TaskTree:
    """Hierarchical task state manager.

    Maintains the tree of tasks and provides operations for:
    - Navigating the tree
    - Breaking down tasks
    - Tracking progress
    - Generating prompts
    """

    def __init__(self, root_goal: str):
        self.root = TaskNode(goal=root_goal, status=TaskStatus.ACTIVE)
        self._current = self.root

    @property
    def current(self) -> TaskNode:
        """Get the currently active leaf node."""
        return self._current

    def breakdown(self, subtasks: list[str]) -> TaskNode:
        """Break current task into subtasks, activate first."""
        if not subtasks:
            msg = "Cannot breakdown into empty subtask list"
            raise ValueError(msg)

        for goal in subtasks:
            self._current.add_child(goal)

        # Activate first subtask
        first = self._current.children[0]
        first.mark_active()
        self._current = first
        return first

    def complete(self, summary: str) -> TaskNode | None:
        """Complete current task, move to next sibling or parent.

        Returns the new current node, or None if root is complete.
        """
        self._current.mark_done(summary)

        # Find next: sibling or parent
        parent = self._current.parent
        if parent is None:
            # Root complete
            return None

        # Check for pending siblings
        for child in parent.children:
            if child.status == TaskStatus.PENDING:
                child.mark_active()
                self._current = child
                return child

        # All siblings done, complete parent
        parent_summary = self._summarize_children(parent)
        parent.mark_done(parent_summary)

        # Move up
        if parent.parent:
            self._current = parent.parent
            return self.complete(parent_summary)  # Recursive check
        else:
            return None  # Root complete

    def _summarize_children(self, node: TaskNode) -> str:
        """Generate summary from completed children."""
        summaries = [c.summary for c in node.children if c.summary]
        if summaries:
            return "; ".join(summaries[:3])  # First 3
        return "completed"

    def add_note(
        self,
        content: str,
        expiry: NoteExpiry = NoteExpiry.ON_DONE,
        turns: int | None = None,
    ) -> Note:
        """Add note to current node."""
        return self._current.add_note(content, expiry, turns)

    def tick_notes(self) -> None:
        """Decrement turn counters on all notes."""
        for node in self._current.path_to_root:
            for note in node.notes:
                note.tick()
            # Remove expired
            node.notes = [n for n in node.notes if not n.should_expire("tick")]

    def format_context(self, include_notes: bool = True) -> str:
        """Generate context string for prompt."""
        return self._current.format_path(include_notes=include_notes)

    def to_dict(self) -> dict:
        """Serialize tree to dict for persistence."""
        return self._node_to_dict(self.root)

    def _node_to_dict(self, node: TaskNode) -> dict:
        """Serialize a single node."""
        return {
            "goal": node.goal,
            "status": node.status.name,
            "summary": node.summary,
            "description": node.description,
            "children": [self._node_to_dict(c) for c in node.children],
            "notes": [
                {
                    "content": n.content,
                    "expiry": n.expiry.name,
                    "turns_remaining": n.turns_remaining,
                }
                for n in node.notes
            ],
            "metadata": node.metadata,
            "sandbox_scope": str(node.sandbox_scope) if node.sandbox_scope else None,
        }

    @classmethod
    def from_dict(cls, data: dict) -> TaskTree:
        """Deserialize tree from dict."""
        tree = cls(data["goal"])
        tree.root = cls._node_from_dict(data)
        tree._current = cls._find_active(tree.root) or tree.root
        return tree

    @classmethod
    def _node_from_dict(cls, data: dict, parent: TaskNode | None = None) -> TaskNode:
        """Deserialize a single node."""
        scope = data.get("sandbox_scope")
        node = TaskNode(
            goal=data["goal"],
            status=TaskStatus[data["status"]],
            summary=data.get("summary"),
            description=data.get("description"),
            parent=parent,
            metadata=data.get("metadata", {}),
            sandbox_scope=Path(scope) if scope else None,
        )
        node.notes = [
            Note(
                content=n["content"],
                expiry=NoteExpiry[n["expiry"]],
                turns_remaining=n.get("turns_remaining"),
            )
            for n in data.get("notes", [])
        ]
        node.children = [cls._node_from_dict(c, node) for c in data.get("children", [])]
        return node

    @classmethod
    def _find_active(cls, node: TaskNode) -> TaskNode | None:
        """Find the active leaf node."""
        if node.status == TaskStatus.ACTIVE and node.is_leaf:
            return node
        for child in node.children:
            active = cls._find_active(child)
            if active:
                return active
        return None


__all__ = [
    "Note",
    "NoteExpiry",
    "TaskNode",
    "TaskStatus",
    "TaskTree",
]
