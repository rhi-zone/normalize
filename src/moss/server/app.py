"""Moss Server application.

This module provides the FastAPI application for serving MossAPI
over HTTP with WebSocket support for streaming.

The server uses generated routes from moss.gen.http, ensuring the
HTTP API is always in sync with the library API.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

from moss.gen.http import HTTPExecutor, HTTPGenerator
from moss.server.state import ServerState


def create_app(root: str | Path = ".") -> Any:
    """Create a FastAPI application for Moss.

    Uses generated routes from HTTPGenerator, with additional endpoints
    for cache management and WebSocket streaming.

    Args:
        root: Project root directory

    Returns:
        FastAPI application instance

    Raises:
        ImportError: If FastAPI is not installed
    """
    try:
        from fastapi import FastAPI, WebSocket, WebSocketDisconnect
        from fastapi.middleware.cors import CORSMiddleware
    except ImportError as e:
        raise ImportError(
            "FastAPI is required for the server. Install with: pip install 'moss[server]'"
        ) from e

    root_path = Path(root).resolve()
    state = ServerState(root=root_path)
    executor = HTTPExecutor(root_path)

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        """Manage application lifespan."""
        app.state.moss_state = state
        app.state.executor = executor
        yield
        state.invalidate()

    # Generate base app with all API routes
    generator = HTTPGenerator()
    app = generator.generate_app(root_path)

    # Replace lifespan (generator's app doesn't have state management)
    app.router.lifespan_context = lifespan

    # Add CORS middleware
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # ==========================================================================
    # Cache Management Endpoints
    # ==========================================================================

    @app.get("/cache/stats")
    async def cache_stats():
        """Get cache statistics."""
        return state.stats()

    @app.post("/cache/invalidate")
    async def invalidate_cache(pattern: str | None = None):
        """Invalidate cache entries."""
        count = state.invalidate(pattern)
        return {"invalidated": count}

    # ==========================================================================
    # WebSocket Endpoint for Streaming
    # ==========================================================================

    @app.websocket("/ws")
    async def websocket_endpoint(websocket: WebSocket):
        """WebSocket endpoint for streaming operations.

        Accepts JSON messages with format:
            {"operation": "skeleton.extract", "args": {"file_path": "..."}}

        Returns:
            {"status": "success", "operation": "...", "result": ...}
            or
            {"status": "error", "operation": "...", "error": "..."}
        """
        await websocket.accept()
        try:
            while True:
                data = await websocket.receive_json()
                operation = data.get("operation")
                args = data.get("args", {})

                try:
                    result = executor.execute(operation, args)
                    await websocket.send_json(
                        {
                            "status": "success",
                            "operation": operation,
                            "result": result,
                        }
                    )
                except Exception as e:
                    await websocket.send_json(
                        {
                            "status": "error",
                            "operation": operation,
                            "error": str(e),
                        }
                    )
        except WebSocketDisconnect:
            pass

    return app


def run_server(
    root: str | Path = ".",
    host: str = "127.0.0.1",
    port: int = 8000,
    uds: str | Path | None = None,
    **kwargs: Any,
) -> None:
    """Run the Moss server.

    Args:
        root: Project root directory
        host: Host to bind to (ignored if uds is set)
        port: Port to bind to (ignored if uds is set)
        uds: Unix domain socket path (takes precedence over host/port)
        **kwargs: Additional uvicorn arguments
    """
    try:
        import uvicorn
    except ImportError as e:
        raise ImportError(
            "Uvicorn is required for the server. Install with: pip install 'moss[server]'"
        ) from e

    app = create_app(root)

    if uds:
        # Unix domain socket - don't pass host/port
        uvicorn.run(app, uds=str(uds), **kwargs)
    else:
        uvicorn.run(app, host=host, port=port, **kwargs)


def main() -> None:
    """CLI entry point for moss-server."""
    import argparse

    parser = argparse.ArgumentParser(
        prog="moss-server",
        description="Run the Moss API server",
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="Project root directory (default: current directory)",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host to bind to (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8000,
        help="Port to bind to (default: 8000)",
    )
    parser.add_argument(
        "--socket",
        "--uds",
        dest="uds",
        help="Unix domain socket path (overrides --host/--port)",
    )
    parser.add_argument(
        "--reload",
        action="store_true",
        help="Enable auto-reload for development",
    )

    args = parser.parse_args()
    run_server(
        root=args.root,
        host=args.host,
        port=args.port,
        uds=args.uds,
        reload=args.reload,
    )


__all__ = [
    "create_app",
    "main",
    "run_server",
]
