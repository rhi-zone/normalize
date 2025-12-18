"""Shadow Git: Atomic commits on shadow branches with rollback support.

# See: docs/architecture/overview.md
"""

from __future__ import annotations

import asyncio
import subprocess
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from uuid import UUID, uuid4


class GitError(Exception):
    """Git operation failed."""

    def __init__(self, message: str, returncode: int, stderr: str):
        super().__init__(message)
        self.returncode = returncode
        self.stderr = stderr


@dataclass(frozen=True)
class CommitHandle:
    """Reference to a shadow commit."""

    sha: str
    message: str
    timestamp: datetime
    branch: str


@dataclass
class ShadowBranch:
    """A shadow branch for isolated agent work."""

    name: str
    base_branch: str
    repo_path: Path
    commits: list[CommitHandle] = field(default_factory=list)
    _id: UUID = field(default_factory=uuid4)

    @property
    def id(self) -> UUID:
        return self._id


class ShadowGit:
    """Manages shadow branches for atomic, reversible agent operations."""

    def __init__(self, repo_path: Path | str):
        self.repo_path = Path(repo_path).resolve()
        self._branches: dict[str, ShadowBranch] = {}

    async def _run_git(self, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        """Run a git command asynchronously."""
        proc = await asyncio.create_subprocess_exec(
            "git",
            *args,
            cwd=self.repo_path,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()
        stdout_str = stdout.decode().strip()
        stderr_str = stderr.decode().strip()

        if check and proc.returncode != 0:
            raise GitError(
                f"git {' '.join(args)} failed: {stderr_str}",
                proc.returncode or 1,
                stderr_str,
            )

        return subprocess.CompletedProcess(
            args=["git", *args],
            returncode=proc.returncode or 0,
            stdout=stdout_str,
            stderr=stderr_str,
        )

    async def _get_current_branch(self) -> str:
        """Get the current branch name."""
        result = await self._run_git("rev-parse", "--abbrev-ref", "HEAD")
        return result.stdout

    async def _get_head_sha(self) -> str:
        """Get the current HEAD commit SHA."""
        result = await self._run_git("rev-parse", "HEAD")
        return result.stdout

    async def create_shadow_branch(self, name: str | None = None) -> ShadowBranch:
        """Create a new shadow branch from current HEAD."""
        base_branch = await self._get_current_branch()
        branch_name = name or f"shadow/{uuid4().hex[:8]}"

        await self._run_git("checkout", "-b", branch_name)

        branch = ShadowBranch(
            name=branch_name,
            base_branch=base_branch,
            repo_path=self.repo_path,
        )
        self._branches[branch_name] = branch
        return branch

    async def checkout_shadow_branch(self, branch: ShadowBranch) -> None:
        """Switch to a shadow branch."""
        await self._run_git("checkout", branch.name)

    async def commit(
        self,
        branch: ShadowBranch,
        message: str,
        *,
        allow_empty: bool = False,
    ) -> CommitHandle:
        """Create an atomic commit on the shadow branch."""
        # Ensure we're on the right branch
        current = await self._get_current_branch()
        if current != branch.name:
            await self._run_git("checkout", branch.name)

        # Stage all changes
        await self._run_git("add", "-A")

        # Check if there are changes to commit
        status = await self._run_git("status", "--porcelain", check=False)
        if not status.stdout and not allow_empty:
            raise GitError("Nothing to commit", 1, "No changes staged")

        # Create commit
        cmd = ["commit", "-m", message]
        if allow_empty:
            cmd.append("--allow-empty")
        await self._run_git(*cmd)

        sha = await self._get_head_sha()
        handle = CommitHandle(
            sha=sha,
            message=message,
            timestamp=datetime.now(UTC),
            branch=branch.name,
        )
        branch.commits.append(handle)
        return handle

    async def rollback(self, branch: ShadowBranch, steps: int = 1) -> None:
        """Rollback the shadow branch by N commits."""
        if steps < 1:
            raise ValueError("steps must be at least 1")
        if steps > len(branch.commits):
            raise ValueError(f"Cannot rollback {steps} commits; only {len(branch.commits)} exist")

        current = await self._get_current_branch()
        if current != branch.name:
            await self._run_git("checkout", branch.name)

        await self._run_git("reset", "--hard", f"HEAD~{steps}")

        # Update commit list
        branch.commits = branch.commits[:-steps]

    async def rollback_to(self, branch: ShadowBranch, commit: CommitHandle) -> None:
        """Rollback to a specific commit."""
        if commit not in branch.commits:
            raise ValueError("Commit not found in branch history")

        idx = branch.commits.index(commit)
        steps = len(branch.commits) - idx - 1
        if steps > 0:
            await self.rollback(branch, steps)

    async def squash_merge(
        self,
        branch: ShadowBranch,
        message: str | None = None,
    ) -> CommitHandle:
        """Squash merge shadow branch into base branch."""
        if not branch.commits:
            raise GitError("No commits to merge", 1, "Shadow branch has no commits")

        base = branch.base_branch
        merge_msg = message or f"Merge shadow branch {branch.name}"

        # Checkout base branch
        await self._run_git("checkout", base)

        # Squash merge
        await self._run_git("merge", "--squash", branch.name)
        await self._run_git("commit", "-m", merge_msg)

        sha = await self._get_head_sha()
        handle = CommitHandle(
            sha=sha,
            message=merge_msg,
            timestamp=datetime.now(UTC),
            branch=base,
        )

        return handle

    async def abort(self, branch: ShadowBranch) -> None:
        """Abort and delete the shadow branch."""
        base = branch.base_branch

        # Checkout base first
        await self._run_git("checkout", base)

        # Delete shadow branch
        await self._run_git("branch", "-D", branch.name)

        # Remove from tracking
        self._branches.pop(branch.name, None)

    async def diff(self, branch: ShadowBranch) -> str:
        """Get diff of all changes on shadow branch vs base."""
        result = await self._run_git("diff", f"{branch.base_branch}...{branch.name}")
        return result.stdout

    async def diff_stat(self, branch: ShadowBranch) -> str:
        """Get diff stat of changes on shadow branch."""
        result = await self._run_git("diff", "--stat", f"{branch.base_branch}...{branch.name}")
        return result.stdout

    def get_branch(self, name: str) -> ShadowBranch | None:
        """Get a tracked shadow branch by name."""
        return self._branches.get(name)

    @property
    def active_branches(self) -> list[ShadowBranch]:
        """List all active shadow branches."""
        return list(self._branches.values())
