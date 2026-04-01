//! Code generation service for server-less CLI.

use crate::commands::generate::{Backend, InputFormat};
use crate::output::OutputFormatter;
use server_less::cli;
use std::path::PathBuf;

/// Code generation sub-service.
pub struct GenerateService;

/// Report for `normalize generate` commands (methods that write to file or stdout).
///
/// `output` contains the generated code. `path` is the file it was written to when
/// `--output` was specified; when absent, `output` is printed to stdout by the formatter.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct GenerateReport {
    /// Generated code (API client, schema, etc.).
    pub output: String,
    /// Output file path, if the result was written to disk rather than stdout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl OutputFormatter for GenerateReport {
    fn format_text(&self) -> String {
        if self.path.is_some() {
            // File was written, show nothing on stdout (message went to stderr)
            String::new()
        } else {
            self.output.clone()
        }
    }
}

impl GenerateService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(
    name = "generate",
    description = "Generate code from an API spec. Use to scaffold clients or types from OpenAPI definitions."
)]
impl GenerateService {
    /// Generate API client from OpenAPI spec
    ///
    /// Examples:
    ///   normalize generate client api.json -l typescript          # generate TypeScript client
    ///   normalize generate client api.json -l python -o client.py # generate Python client to file
    #[cli(display_with = "display_output")]
    pub fn client(
        &self,
        #[param(positional, help = "OpenAPI spec JSON file")] spec: String,
        #[param(short = 'l', help = "Target language: typescript, python, rust")] lang: String,
        #[param(short = 'o', help = "Output file (stdout if not specified)")] output: Option<
            String,
        >,
    ) -> Result<GenerateReport, String> {
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
            Ok(GenerateReport {
                output: code,
                path: Some(path.clone()),
            })
        } else {
            Ok(GenerateReport {
                output: code,
                path: None,
            })
        }
    }

    /// Generate types/validators from schema
    ///
    /// Examples:
    ///   normalize generate types schema.json -b typescript        # generate TypeScript types
    ///   normalize generate types schema.json -b zod --infer-types # Zod schemas with type inference
    ///   normalize generate types schema.json -b go --package models -o models.go
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub fn types(
        &self,
        #[param(
            positional,
            help = "Input schema file (JSON Schema or OpenAPI), use - for stdin"
        )]
        input: String,
        #[param(short = 'b', help = "Output backend")] backend: Backend,
        #[param(
            short = 'f',
            help = "Input format (auto, json-schema, openapi, typescript)"
        )]
        format: Option<InputFormat>,
        #[param(short = 'o', help = "Output file (stdout if not specified)")] output: Option<
            String,
        >,
        #[param(help = "Export all types (add 'export' keyword)")] export: Option<bool>,
        #[param(help = "Generate type inference (for Zod/Valibot)")] infer_types: bool,
        #[param(help = "Make types readonly/frozen")] readonly: bool,
        #[param(help = "Package name (for Go)")] package: Option<String>,
    ) -> Result<GenerateReport, String> {
        let input_format = format.unwrap_or(InputFormat::Auto);
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
    ///
    /// Examples:
    ///   normalize generate cli-snapshot ./target/debug/myapp              # generate snapshot tests
    ///   normalize generate cli-snapshot ./target/debug/myapp -o tests/cli.rs  # write to file
    #[cli(name = "cli-snapshot", display_with = "display_output")]
    pub fn cli_snapshot(
        &self,
        #[param(positional, help = "Path to the CLI binary")] binary: String,
        #[param(
            short = 'o',
            help = "Output file for the test (stdout if not specified)"
        )]
        output: Option<String>,
        #[param(help = "Binary name to use in test (defaults to file stem)")] name: Option<String>,
    ) -> Result<GenerateReport, String> {
        crate::commands::generate::run_cli_snapshot_service(
            PathBuf::from(&binary),
            output.map(PathBuf::from),
            name,
        )
    }
}
