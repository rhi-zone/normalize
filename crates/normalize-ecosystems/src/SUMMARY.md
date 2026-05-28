# normalize-ecosystems/src

Source for the normalize-ecosystems crate.

`lib.rs` defines the `Ecosystem` trait, all shared data types, and the default `query()` / `detect_tool()` / `find_tool()` implementations. `ecosystems/` contains one module per ecosystem. `cache.rs` provides the on-disk JSON cache keyed by ecosystem + package name. `http.rs` provides a thin ureq wrapper with gzip support used by the registry-fetching ecosystem impls.

`symbol_docs.rs` defines `SymbolDoc` (structured docs for the `normalize docs` command) and `DocFormat` (`Markdown`/`Rst`/`Html`/`PlainText`). Doc bodies are stored **source-native**: the body lives in `doc_body` tagged with `doc_format`, and rendering to display Markdown happens at the output layer (`render_symbol_doc` in the main crate), never at fetch time. `local_docs.rs` extracts Rust `///` docs (→ `DocFormat::Markdown`); `docs_rs.rs` fetches rustdoc docblocks as raw HTML fragments (→ `DocFormat::Html`) and exposes the `html_to_markdown` / `strip_html_tags` / `decode_html_entities` / `normalize_blank_lines` helpers publicly so the output layer can render them. `SymbolDoc::kg_id()` derives its prefix from `language` (`docs-rust-…`).
