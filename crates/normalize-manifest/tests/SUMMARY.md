# normalize-manifest/tests

Integration tests for normalize-manifest parsers.

`real_world.rs` contains tests that parse real manifest files from the `fixtures/` subdirectory and assert on the extracted package name, version, and dependency list. Each fixture subdirectory corresponds to a well-known open-source project and exercises a different manifest format.
