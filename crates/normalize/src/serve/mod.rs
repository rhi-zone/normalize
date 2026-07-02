//! Server commands for normalize (MCP, HTTP, LSP).
//!
//! Servers expose normalize functionality over various protocols.

use serde::Deserialize;

#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "lsp")]
pub mod lsp;
pub mod mcp;

/// Serve configuration from config.toml.
#[derive(
    Debug, Clone, Deserialize, serde::Serialize, Default, schemars::JsonSchema, server_less::Config,
)]
#[serde(default)]
pub struct ServeConfig {
    /// Default HTTP port (overridden by --port).
    pub http_port: Option<u16>,
    /// HTTP host to bind to.
    pub http_host: Option<String>,
    /// Debounce interval for LSP fact diagnostics in milliseconds. Default: 1500
    pub fact_debounce_ms: Option<u64>,
}

impl ServeConfig {
    pub fn http_port(&self) -> u16 {
        self.http_port.unwrap_or(8080)
    }

    pub fn http_host(&self) -> &str {
        self.http_host.as_deref().unwrap_or("127.0.0.1")
    }

    pub fn fact_debounce_ms(&self) -> u64 {
        self.fact_debounce_ms.unwrap_or(1500)
    }
}
