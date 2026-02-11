# Interface Generators

Normalize follows a **library-first design**: the core is `NormalizeAPI` in `src/normalize/normalize_api.py`, and all interfaces (CLI, HTTP, MCP, LSP, TUI, gRPC) are generated from it.

## Overview

```
NormalizeAPI (normalize_api.py)
    │
    ▼
introspect_api() → list[SubAPI]
    │
    ├─► MCPGenerator  → MCP tools (mcp_server.py)
    ├─► HTTPGenerator → FastAPI routes (server/app.py)
    ├─► CLIGenerator  → argparse commands
    ├─► LSPGenerator  → workspace commands
    ├─► TUIGenerator  → Textual UI screens
    └─► GRPCGenerator → Protocol Buffers + servicer
```

## How It Works

### 1. Introspection (`gen/introspect.py`)

The `introspect_api()` function analyzes `NormalizeAPI` to extract:

- **SubAPIs**: Classes like `SkeletonAPI`, `HealthAPI`, `RAGAPI`
- **Methods**: Public methods with their signatures
- **Parameters**: Names, types, defaults, docstrings
- **Return types**: For serialization hints

```python
from normalize.gen import introspect_api

sub_apis = introspect_api()
# Returns: [SubAPI(name="skeleton", methods=[...]), SubAPI(name="health", ...)]
```

### 2. Adding a New Tool

To add a new MCP/HTTP/CLI tool:

1. **Add API class** to `normalize_api.py`:
   ```python
   @dataclass
   class MyNewAPI:
       """API for doing something useful."""
       root: Path

       def my_method(self, arg: str) -> str:
           """Do the thing.

           Args:
               arg: The input argument

           Returns:
               The result string
           """
           return f"Result: {arg}"
   ```

2. **Add accessor** to `NormalizeAPI`:
   ```python
   @property
   def my_new(self) -> MyNewAPI:
       """Access my new functionality."""
       return MyNewAPI(root=self.root)
   ```

3. **Register in introspect.py**:
   ```python
   # In sub_apis dict:
   "my_new": (MyNewAPI, "my_new"),
   ```

4. **Regenerate interfaces**:
   ```bash
   normalize gen --target=mcp   # Regenerate MCP server
   normalize gen --target=http  # Regenerate HTTP routes
   normalize gen --target=all   # Regenerate everything
   ```

### 3. Generators

#### MCP Generator (`gen/mcp.py`)

Generates MCP tool definitions for the Model Context Protocol:

```python
from normalize.gen import generate_mcp_definitions

tools = generate_mcp_definitions()
# Returns: list of MCP Tool objects with schemas
```

The MCP server in `mcp_server.py` uses these definitions directly.

#### HTTP Generator (`gen/http.py`)

Generates FastAPI routes:

```python
from normalize.gen import generate_http, generate_openapi

routes = generate_http()  # FastAPI router
openapi_spec = generate_openapi()  # OpenAPI JSON
```

#### CLI Generator (`gen/cli.py`)

Generates argparse command structure:

```python
from normalize.gen import generate_cli

parser = generate_cli()
```

#### TUI Generator (`gen/tui.py`)

Generates Textual terminal UI:

```python
from normalize.gen import run_tui

run_tui()  # Launches interactive TUI
```

#### LSP Generator (`gen/lsp.py`)

Generates Language Server Protocol workspace commands:

```python
from normalize.gen import generate_lsp_commands

commands = generate_lsp_commands()
```

#### gRPC Generator (`gen/grpc.py`)

Generates Protocol Buffers and Python servicer:

```python
from normalize.gen import generate_proto, generate_servicer_code

proto_content = generate_proto()
servicer_code = generate_servicer_code()
```

## Serialization (`gen/serialize.py`)

All generators use shared serialization for consistent output:

- `to_compact()`: Token-efficient single-line format
- `to_dict()`: JSON-serializable dictionary
- `to_markdown()`: Human-readable markdown

## Drift Detection

The CI checks that generated specs match committed versions:

```bash
# Check for drift
python scripts/check_gen_drift.py

# Auto-update specs
normalize gen --target=mcp --output=specs/mcp_tools.json
normalize gen --target=openapi --output=specs/openapi.json
```

Pre-commit hooks automatically update specs when API changes.

## Design Principles

1. **Single Source of Truth**: `NormalizeAPI` is canonical; interfaces derive from it
2. **Docstrings are Documentation**: Method docstrings become tool descriptions
3. **Type Hints are Schemas**: Python types become JSON schemas
4. **Consistent Serialization**: All outputs use the same formatting
5. **No Manual Sync**: Regenerate, don't manually update interface code
