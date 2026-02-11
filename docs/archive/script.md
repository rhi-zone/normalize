# normalize script

Run Lua scripts with normalize bindings.

## Usage

```bash
normalize script <PATH>
normalize <PATH>           # Direct invocation for .lua files
normalize @<script-name>   # Run from .normalize/scripts/
```

## Examples

```bash
# Run a script
normalize script analyze.lua

# Direct invocation
normalize ./my-script.lua

# Named script from .normalize/scripts/
normalize @todo list
normalize @cleanup
```

## Script Location

Scripts are searched in:
1. Direct path (if provided)
2. `.normalize/scripts/` directory
3. `~/.normalize/scripts/` (global)

## Lua Bindings

Scripts have access to:

```lua
-- File operations
normalize.read(path)
normalize.write(path, content)
normalize.glob(pattern)
normalize.grep(pattern, path)

-- Tree-sitter
normalize.parse(path)
normalize.skeleton(path)

-- Subprocess
normalize.exec(cmd, args)

-- Output
normalize.print(...)
normalize.json(value)
```

See `docs/scripting.md` for full API.
