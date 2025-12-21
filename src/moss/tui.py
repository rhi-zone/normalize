"""TUI Interface: Interactive terminal UI for Moss.

Uses Textual for a modern, reactive terminal experience.
"""

from __future__ import annotations

from enum import Enum, auto
from typing import TYPE_CHECKING, ClassVar

try:
    from textual.app import App, ComposeResult
    from textual.containers import Container, Horizontal, Vertical
    from textual.reactive import reactive
    from textual.widgets import Footer, Header, Input, Static, Tree
    from textual.widgets.tree import TreeNode
except ImportError:
    # TUI dependencies not installed
    class App:
        pass

    class ComposeResult:
        pass


if TYPE_CHECKING:
    from moss.moss_api import MossAPI
    from moss.task_tree import TaskNode, TaskTree


class AgentMode(Enum):
    """Current operating mode of the agent UI."""

    PLAN = auto()  # Planning next steps
    READ = auto()  # Code exploration and search
    WRITE = auto()  # Applying changes and refactoring
    DIFF = auto()  # Reviewing shadow git changes


class ModeIndicator(Static):
    """Widget to display the current agent mode."""

    mode = reactive(AgentMode.PLAN)

    def render(self) -> str:
        colors = {
            AgentMode.PLAN: "blue",
            AgentMode.READ: "green",
            AgentMode.WRITE: "red",
            AgentMode.DIFF: "magenta",
        }
        color = colors.get(self.mode, "white")
        return f"Mode: [{color} b]{self.mode.name}[/]"


class TaskTreeWidget(Tree[str]):
    """Widget for visualizing the task tree."""

    def update_from_tree(self, task_tree: TaskTree) -> None:
        """Update the widget content from a TaskTree instance."""
        self.clear()
        root = self.root
        root.label = task_tree.root.goal
        self._add_node(root, task_tree.root)
        root.expand()

    def _add_node(self, tree_node: TreeNode[str], task_node: TaskNode) -> None:
        """Recursively add nodes to the tree widget."""
        for child in task_node.children:
            status_icon = "✓" if child.status.name == "DONE" else "→"
            label = f"{status_icon} {child.goal}"
            if child.summary:
                label += f" ({child.summary})"

            new_node = tree_node.add(label, expand=True)
            self._add_node(new_node, child)


class MossTUI(App):
    """The main Moss TUI application."""

    CSS = """
    Screen {
        background: $surface;
    }

    #main-container {
        height: 1fr;
    }

    #sidebar {
        width: 30%;
        height: 1fr;
        border-right: tall $primary;
        background: $surface-darken-1;
    }

    #content-area {
        width: 70%;
        height: 1fr;
        padding: 1;
    }

    #command-input {
        dock: bottom;
        margin: 1;
    }

    .log-entry {
        margin-bottom: 1;
        padding: 0 1;
        border-left: solid $accent;
    }

    #git-view {
        display: none;
    }

    #diff-view {
        height: 1fr;
        border: solid $secondary;
    }

    #history-tree {
        height: 30%;
        border: solid $secondary;
    }

    ModeIndicator {
        background: $surface-lighten-1;
        padding: 0 1;
        text-align: center;
        border: round $primary;
        margin: 0 1;
    }
    """

    BINDINGS: ClassVar[list[tuple[str, str, str]]] = [
        ("q", "quit", "Quit"),
        ("d", "toggle_dark", "Toggle Dark Mode"),
        ("shift+tab", "next_mode", "Next Mode"),
    ]

    mode = reactive(AgentMode.PLAN)

    def __init__(self, api: MossAPI):
        super().__init__()
        self.api = api
        self._task_tree: TaskTree | None = None

    def compose(self) -> ComposeResult:
        """Create child widgets for the app."""
        from textual.widgets import RichLog

        yield Header(show_clock=True)
        yield Horizontal(ModeIndicator(id="mode-indicator"), id="header-bar", height="auto")
        yield Container(
            Horizontal(
                Vertical(
                    Static("Task Tree", classes="sidebar-header"),
                    TaskTreeWidget("Tasks", id="task-tree"),
                    id="sidebar",
                ),
                Vertical(
                    Static("Agent Log", id="content-header"),
                    Container(id="log-view"),
                    Container(
                        Static("Shadow Git History", classes="sidebar-header"),
                        Tree("Commits", id="history-tree"),
                        Static("Diff", classes="sidebar-header"),
                        RichLog(id="diff-view", highlight=True, markup=True),
                        id="git-view",
                    ),
                    id="content-area",
                ),
                id="main-container",
            ),
            Input(placeholder="Enter command...", id="command-input"),
        )
        yield Footer()

    def on_mount(self) -> None:
        """Called when the app is mounted."""
        self.title = "Moss TUI"
        self.sub_title = f"Project: {self.api.root.name}"
        self.query_one("#command-input").focus()

    def watch_mode(self, mode: AgentMode) -> None:
        """React to mode changes."""
        self.query_one("#mode-indicator").mode = mode
        # Update input placeholder based on mode
        placeholders = {
            AgentMode.PLAN: "What is the plan? (e.g. breakdown...)",
            AgentMode.READ: "Explore codebase... (e.g. skeleton, grep, expand)",
            AgentMode.WRITE: "Modify code... (e.g. write, replace, insert)",
            AgentMode.DIFF: "Review changes... (revert <file> <line> to undo)",
        }
        self.query_one("#command-input").placeholder = placeholders.get(mode, "Enter command...")

        # Toggle views
        log_view = self.query_one("#log-view")
        git_view = self.query_one("#git-view")
        header = self.query_one("#content-header")

        if mode == AgentMode.DIFF:
            log_view.display = False
            git_view.display = True
            header.update("Shadow Git")
            self._update_git_view()
        else:
            log_view.display = True
            git_view.display = False
            header.update("Agent Log")

    async def _update_git_view(self) -> None:
        """Fetch and display shadow git data."""
        try:
            # Get current shadow branch diff
            # In a real TUI we'd track the current branch
            diff = await self.api.shadow_git.get_diff("shadow/current")
            diff_view = self.query_one("#diff-view")
            diff_view.clear()
            diff_view.write(diff)

            # Update history (hunks)
            hunks = await self.api.shadow_git.get_hunks("shadow/current")
            history = self.query_one("#history-tree")
            history.clear()
            root = history.root
            root.label = "Current Hunks"
            for hunk in hunks:
                label = f"{hunk['file_path']}:{hunk['new_start']} ({hunk['symbol'] or 'no symbol'})"
                root.add_leaf(label)
            root.expand()
        except Exception as e:
            self._log(f"Failed to fetch git data: {e}")

    def action_next_mode(self) -> None:
        """Switch to the next mode."""
        modes = list(AgentMode)
        current_idx = modes.index(self.mode)
        next_idx = (current_idx + 1) % len(modes)
        self.mode = modes[next_idx]
        self._log(f"Switched to {self.mode.name} mode")

    async def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle command input."""
        command = event.value.strip()
        if not command:
            return

        self.query_one("#command-input").value = ""
        self._log(f"[{self.mode.name}] {command}")

        # TODO: Integrate with AgentLoop or DWIM
        if command == "exit":
            self.exit()

    def _log(self, message: str) -> None:
        """Add a message to the log view."""
        log_view = self.query_one("#log-view")
        log_view.mount(Static(message, classes="log-entry"))
        log_view.scroll_end()


def run_tui(api: MossAPI) -> None:
    """Run the Moss TUI."""
    try:
        from textual.app import App as _App  # noqa: F401
    except ImportError:
        print("Error: textual not installed. Install with: pip install 'moss[tui]'")
        return

    app = MossTUI(api)
    app.run()
