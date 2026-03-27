# External Native Rule Protocol

Custom native Rust rules without ABI concerns.

## Purpose

The `abi_stable`/dylib rule pack system was removed due to heap corruption issues at the
allocator boundary. The replacement for custom native rules is an external-process protocol:
the host sends `Relations` as a zero-copy rkyv archive over stdin, and the external process
writes NDJSON `Diagnostic` objects on stdout.

This gives external-process isolation (no shared allocator, no ABI versioning concerns)
without the full JSON serialization cost of SARIF — cheap enough for pre-commit use.

## Wire Protocol

### Input: length-prefixed rkyv archive

The host writes a 4-byte little-endian `u32` length prefix followed by the rkyv-serialized
`Relations` bytes:

```
[u32 LE: byte length][rkyv archive bytes...]
```

The external process reads exactly `length` bytes from stdin and deserializes using
`rkyv::from_bytes::<Relations>(&buf)`.

### Output: NDJSON diagnostics

The external process writes one JSON-serialized `Diagnostic` per line to stdout (NDJSON).
Lines must be valid JSON matching the `Diagnostic` struct. The process exits 0 on success.

```json
{"rule_id":"my-rule","level":"Warning","message":"Found an issue","location":{"file":"src/lib.rs","line":42,"column":null},"related":[],"suggestion":null}
```

## Build Template

A minimal external rule binary in Rust:

```rust
use std::io::{self, Read};
use normalize_facts_rules_api::{Relations, Diagnostic};

fn main() -> anyhow::Result<()> {
    // Read length prefix
    let mut len_buf = [0u8; 4];
    io::stdin().read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;

    // Read archive bytes
    let mut buf = vec![0u8; len];
    io::stdin().read_exact(&mut buf)?;

    // Deserialize (zero-copy)
    let relations = rkyv::from_bytes::<Relations>(&buf)
        .map_err(|e| anyhow::anyhow!("rkyv deserialize error: {e}"))?;

    // Run your rule logic
    let diagnostics = run_rule(&relations);

    // Write NDJSON output
    for diag in diagnostics {
        println!("{}", serde_json::to_string(&diag)?);
    }

    Ok(())
}

fn run_rule(relations: &Relations) -> Vec<Diagnostic> {
    // Inspect relations.symbols, relations.imports, relations.calls, etc.
    vec![]
}
```

Add to `Cargo.toml`:

```toml
[dependencies]
normalize-facts-rules-api = "0.1"
rkyv = { version = "0.8", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

## Registration (not yet implemented)

The registration mechanism for external rules in normalize config is not yet defined.
Placeholder: a future `normalize.toml` entry will point to an external rule binary path,
and normalize will invoke it as a subprocess during `normalize rules run`.

See `docs/rules.md` for the overall rules engine architecture and `docs/fact-rules.md` for
the Datalog rule pack system.
