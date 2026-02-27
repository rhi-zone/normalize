//! Serve sub-service for server-less CLI.

use server_less::cli;
use std::path::PathBuf;

/// Serve sub-service (MCP, HTTP, LSP).
pub struct ServeService;

#[cli]
impl ServeService {
    /// Start MCP server for LLM integration (stdio transport)
    pub fn mcp(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<(), String> {
        let root_path = root.as_deref().map(PathBuf::from);
        let exit = crate::serve::mcp::cmd_serve_mcp(root_path.as_deref());
        if exit != 0 {
            Err(format!("MCP server exited with code {}", exit))
        } else {
            Ok(())
        }
    }

    /// Start HTTP server (REST API)
    pub fn http(
        &self,
        #[param(short = 'p', help = "Port to listen on [default: 8080]")] port: Option<u16>,
        #[param(help = "Output OpenAPI spec and exit (don't start server)")] openapi: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<serde_json::Value, String> {
        let root_path = root
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        if openapi {
            use crate::serve::http::ApiDoc;
            use utoipa::OpenApi;
            let spec = ApiDoc::openapi();
            return Ok(serde_json::to_value(spec).unwrap_or(serde_json::Value::Null));
        }

        let config = crate::config::NormalizeConfig::load(&root_path);
        let effective_port = port.unwrap_or_else(|| config.serve.http_port());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let exit = rt.block_on(crate::serve::http::run_http_server(
            &root_path,
            effective_port,
        ));
        if exit != 0 {
            Err(format!("HTTP server exited with code {}", exit))
        } else {
            Ok(serde_json::Value::Null)
        }
    }

    /// Start LSP server for IDE integration
    pub fn lsp(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<(), String> {
        let root_path = root.as_deref().map(PathBuf::from);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let exit = rt.block_on(crate::serve::lsp::run_lsp_server(root_path.as_deref()));
        if exit != 0 {
            Err(format!("LSP server exited with code {}", exit))
        } else {
            Ok(())
        }
    }
}
