# normalize docs

Fetch upstream symbol documentation into LLM context.

Retrieves the current documentation for a library symbol and outputs a Markdown
block ready to paste into an LLM prompt. Patches training-cutoff blind spots:
the docs reflect the version actually in use, not whatever the model memorized.
Results are cached in the knowledge graph (`.normalize/kg/`) so repeat lookups
are instant and offline.

## Usage

```bash
normalize docs <symbol> [--ecosystem <name>] [--root <dir>] [--no-cache]
```

The symbol is package-qualified and the syntax is ecosystem-specific (see
below). Append `@version` to pin a specific version; otherwise the installed
version (from the project's lockfile / installed packages) is used, falling
back to the latest published version.

## Multi-language support

`normalize docs` works across ecosystems. It auto-detects the project's
ecosystem from the current directory; pass `--ecosystem`/`-e` to select or
disambiguate.

| Ecosystem | Symbol syntax | Examples |
|-----------|---------------|----------|
| Rust (`cargo`) | `crate::path::Symbol` | `serde::Serialize`, `tokio::sync::Mutex`, `serde` (crate root) |
| Go (`go`) | `import/path#Symbol` or `pkg.Symbol` | `encoding/json#Marshal`, `fmt.Println` |
| Python (`python`) | `package.path.Symbol` | `requests.Session`, `requests.adapters.HTTPAdapter` |

Symbol-parsing conventions per ecosystem:

- **Rust** — standard `::`-separated path; the first segment is the crate.
- **Go** — split on `#` (`import/path#Symbol`); failing that, the last `.`
  (`fmt.Println` → package `fmt`, symbol `Println`). A bare symbol with neither
  is rejected.
- **Python** — the first dotted segment is the package; the dotted remainder is
  the symbol path (`requests.adapters.HTTPAdapter` → package `requests`, symbol
  `adapters.HTTPAdapter`). A bare name with no `.` is rejected.

## Where the docs come from

For each ecosystem, `docs` tries a **local source** first, then falls back to a
**remote package source archive** — it does not scrape documentation websites
for Go or Python:

| Ecosystem | Installed (local) | Uninstalled (remote) |
|-----------|-------------------|----------------------|
| Rust | Cargo source in `~/.cargo` | docs.rs HTML |
| Go | module cache (`$GOMODCACHE`) / SDK (`$GOROOT/src`) | module proxy `{module}/@v/{version}.zip` |
| Python | venv `site-packages` / stdlib | PyPI sdist archive |

Remote Go/Python fetches download and extract the package's source archive into
`~/.cache/normalize/sources/`, then parse docstrings and signatures directly
from the source tree. This means docs for uninstalled packages reflect the real
upstream source, not a rendered docs site.

## Doc bodies and `--json`

Doc bodies are stored **source-native**: the raw text lives in `doc_body`,
tagged with a `doc_format` field. The text renderer interprets the body
according to that tag.

| `doc_format` | Source | Rendering |
|--------------|--------|-----------|
| `markdown` | Rust `///` comments | verbatim |
| `html` | docs.rs docblocks | converted to Markdown at the output layer |
| `rst` | (reserved for reStructuredText) | verbatim |
| `plaintext` | Go / Python docstrings | verbatim |

`--json` output carries the full structured `SymbolDoc` (including `doc_body`
and `doc_format`) plus `from_cache`, so programmatic consumers can render the
body however they like.

## Options

- `-e`, `--ecosystem <name>` — ecosystem to query (e.g. `cargo`, `go`,
  `python`). Auto-detected from the project when omitted. Errors if no
  docs-capable ecosystem is detected, or if more than one is (pass `-e` to
  disambiguate).
- `-r`, `--root <dir>` — root directory for lockfile / installed-version
  lookup (defaults to the current directory).
- `--no-cache` — bypass the local knowledge-graph cache and always fetch from
  source.

## Examples

```bash
# Rust
normalize docs serde::Serialize
normalize docs tokio::sync::Mutex
normalize docs serde                            # crate-level docs
normalize docs serde::Serialize@1.0.193         # pin a specific version
normalize docs serde::Serialize --no-cache      # bypass local cache

# Go
normalize docs -e go encoding/json#Marshal
normalize docs -e go fmt.Println

# Python
normalize docs -e python requests.Session
normalize docs -e python requests.adapters.HTTPAdapter
```
