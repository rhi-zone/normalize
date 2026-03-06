# src/serve

Server implementations that expose normalize functionality over network protocols. Modules: `http.rs` (HTTP REST API server), `lsp.rs` (Language Server Protocol server), `mcp.rs` (Model Context Protocol server for LLM integration via stdio). `mod.rs` defines `ServeConfig` (port/host from normalize.toml) and the `ServeArgs`/`ServeProtocol` clap types. The `mcp` feature flag gates MCP support. All three protocols expose the same underlying code intelligence data.
