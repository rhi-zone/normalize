# normalize init

Initialize normalize in a project directory.

## Usage

```bash
normalize init [--index]
```

## What It Does

1. Creates `.normalize/` directory
2. Creates `.normalize/config.toml` with defaults
3. Detects TODO files (TODO.md, TASKS.md, etc.) and adds to aliases
4. Updates `.gitignore` with normalize entries

## Options

- `--index` - Also build the file index after init

## Generated Files

### .normalize/config.toml

```toml
# Normalize configuration
# See: https://github.com/rhi-zone/normalize

[daemon]
# enabled = true
# auto_start = true

[analyze]
# clones = true

# [analyze.weights]
# health = 1.0
# complexity = 0.5
# security = 2.0
# clones = 0.3

[aliases]
todo = ["TODO.md"]  # If TODO.md exists
```

### .gitignore Entries

```gitignore
# Normalize - ignore .normalize/ in subdirectories entirely
**/.normalize/

# Root .normalize/ - ignore all but config/allow files
/.normalize/*
!/.normalize/config.toml
!/.normalize/duplicate-functions-allow
!/.normalize/duplicate-types-allow
!/.normalize/hotspots-allow
!/.normalize/large-files-allow
```

## Idempotent

Running `normalize init` multiple times is safe:
- Skips existing files
- Only adds missing gitignore entries
- Reports what was created/skipped
