//! Server commands for moss (MCP, HTTP, LSP).
//!
//! Servers expose moss functionality over various protocols.

use clap::{Args, Subcommand};
use std::path::PathBuf;

pub mod http;
pub mod lsp;
pub mod mcp;

/// Serve command arguments
#[derive(Args)]
pub struct ServeArgs {
    #[command(subcommand)]
    pub protocol: ServeProtocol,

    /// Root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    pub root: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum ServeProtocol {
    /// Start MCP server for LLM integration (stdio transport)
    Mcp,

    /// Start HTTP server (REST API)
    Http {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Output OpenAPI spec and exit (don't start server)
        #[arg(long)]
        openapi: bool,
    },

    /// Start LSP server for IDE integration
    Lsp,
}

/// Run the serve command
pub fn run(args: ServeArgs, json: bool) -> i32 {
    match args.protocol {
        ServeProtocol::Mcp => mcp::cmd_serve_mcp(args.root.as_deref(), json),
        ServeProtocol::Http { port, openapi } => {
            if openapi {
                // Output OpenAPI spec and exit
                use http::ApiDoc;
                use utoipa::OpenApi;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap()
                );
                0
            } else {
                let root = args.root.unwrap_or_else(|| PathBuf::from("."));
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(http::run_http_server(&root, port))
            }
        }
        ServeProtocol::Lsp => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(lsp::run_lsp_server(args.root.as_deref()))
        }
    }
}
