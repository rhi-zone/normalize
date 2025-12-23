"""moss-orchestration: Agent loops, sessions, and workflows.

Provides the execution layer for agents:
- Session management with checkpointing
- Driver protocol for agent decision-making
- Shadow git for safe code changes
- Workflow execution

Example:
    from moss_orchestration import Session, Agent
    from moss_orchestration.drivers import LLMDriver

    session = Session.create(task="Fix the auth bug")
    agent = Agent(session=session, driver=LLMDriver())
    await agent.run()
"""

# Re-export key types
from .session import Session, SessionManager, SessionStatus
from .drivers import Driver, Action, ActionResult, Context

__all__ = [
    "Session",
    "SessionManager",
    "SessionStatus",
    "Driver",
    "Action",
    "ActionResult",
    "Context",
]
