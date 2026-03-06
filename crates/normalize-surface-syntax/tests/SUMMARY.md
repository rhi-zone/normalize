# normalize-surface-syntax/tests

Snapshot tests for surface-syntax readers and translation roundtrips.

`snapshots.rs` tests Lua and TypeScript readers against insta snapshots, covering function declarations, binary expressions, control flow, table literals, method calls, and varargs. Also tests try/catch translation via the Lua try-catch module. Snapshots are stored in `snapshots/` as `.snap` files (52 total).
