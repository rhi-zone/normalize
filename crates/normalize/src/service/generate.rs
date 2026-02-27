//! Code generation service for server-less CLI.

use crate::commands::generate::{Backend, InputFormat};
use server_less::cli;
use std::path::PathBuf;

/// Code generation sub-service.
pub struct GenerateService;

/// Generation result (for methods that write to file or stdout).
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct GenerateResult {
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl std::fmt::Display for GenerateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_some() {
            // File was written, show nothing on stdout (message went to stderr)
            Ok(())
        } else {
            write!(f, "{}", self.output)
        }
    }
}

#[cli(name = "generate", about = "Generate code from API spec")]
impl GenerateService {
    /// Generate API client from OpenAPI spec
    pub fn client(
        &self,
        #[param(positional, help = "OpenAPI spec JSON file")] spec: String,
        #[param(short = 'l', help = "Target language: typescript, python, rust")] lang: String,
        #[param(short = 'o', help = "Output file (stdout if not specified)")] output: Option<
            String,
        >,
    ) -> Result<GenerateResult, String> {
        let spec_path = PathBuf::from(&spec);
        let generator = normalize_openapi::find_generator(&lang).ok_or_else(|| {
            let mut msg = format!("Unknown language: {}. Available:", lang);
            for (l, variant) in normalize_openapi::list_generators() {
                msg.push_str(&format!("\n  {} ({})", l, variant));
            }
            msg
        })?;

        let content = std::fs::read_to_string(&spec_path)
            .map_err(|e| format!("Failed to read {}: {}", spec, e))?;
        let spec_json: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let code = generator.generate(&spec_json);

        if let Some(ref path) = output {
            std::fs::write(path, &code).map_err(|e| format!("Failed to write {}: {}", path, e))?;
            eprintln!("Generated {}", path);
            Ok(GenerateResult {
                output: code,
                path: Some(path.clone()),
            })
        } else {
            Ok(GenerateResult {
                output: code,
                path: None,
            })
        }
    }

    /// Generate types/validators from schema
    #[allow(clippy::too_many_arguments)]
    pub fn types(
        &self,
        #[param(
            positional,
            help = "Input schema file (JSON Schema or OpenAPI), use - for stdin"
        )]
        input: String,
        #[param(short = 'b', help = "Output backend")] backend: String,
        #[param(
            short = 'f',
            help = "Input format (auto, json-schema, openapi, typescript)"
        )]
        format: Option<String>,
        #[param(short = 'o', help = "Output file (stdout if not specified)")] output: Option<
            String,
        >,
        #[param(help = "Export all types (add 'export' keyword)")] export: Option<bool>,
        #[param(help = "Generate type inference (for Zod/Valibot)")] infer_types: bool,
        #[param(help = "Make types readonly/frozen")] readonly: bool,
        #[param(help = "Package name (for Go)")] package: Option<String>,
    ) -> Result<GenerateResult, String> {
        let input_format: InputFormat = format
            .as_deref()
            .unwrap_or("auto")
            .parse()
            .map_err(|e: String| e)?;
        let backend: Backend = backend.parse().map_err(|e: String| e)?;
        let export = export.unwrap_or(true);
        let package = package.unwrap_or_else(|| "types".to_string());

        let input_path = PathBuf::from(&input);

        crate::commands::generate::run_types_service(
            input_path,
            input_format,
            backend,
            output.map(PathBuf::from),
            export,
            infer_types,
            readonly,
            package,
        )
    }

    /// Generate CLI snapshot tests for a binary
    #[cli(name = "cli-snapshot")]
    pub fn cli_snapshot(
        &self,
        #[param(positional, help = "Path to the CLI binary")] binary: String,
        #[param(
            short = 'o',
            help = "Output file for the test (stdout if not specified)"
        )]
        output: Option<String>,
        #[param(help = "Binary name to use in test (defaults to file stem)")] name: Option<String>,
    ) -> Result<GenerateResult, String> {
        crate::commands::generate::run_cli_snapshot_service(
            PathBuf::from(&binary),
            output.map(PathBuf::from),
            name,
        )
    }
}
