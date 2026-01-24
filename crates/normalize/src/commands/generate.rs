//! Generate command - code generation from API specs and schemas.

use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Generate command arguments
#[derive(Args)]
pub struct GenerateArgs {
    #[command(subcommand)]
    pub target: GenerateTarget,
}

#[derive(Subcommand)]
pub enum GenerateTarget {
    /// Generate API client from OpenAPI spec (legacy)
    Client {
        /// OpenAPI spec JSON file
        spec: PathBuf,

        /// Target language: typescript, python, rust
        #[arg(short, long)]
        lang: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate types from JSON Schema (legacy)
    Types {
        /// JSON Schema file
        schema: PathBuf,

        /// Root type name
        #[arg(short, long, default_value = "Root")]
        name: String,

        /// Target language: typescript, python, rust
        #[arg(short, long)]
        lang: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate CLI snapshot tests for a binary
    #[command(name = "cli-snapshot")]
    CliSnapshot {
        /// Path to the CLI binary
        binary: PathBuf,

        /// Output file for the test (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Binary name to use in test (defaults to file stem)
        #[arg(long)]
        name: Option<String>,
    },
    /// Generate types/validators from schema (new IR-based)
    #[command(name = "typegen")]
    Typegen {
        /// Input schema file (JSON Schema or OpenAPI), use - for stdin
        input: PathBuf,

        /// Input format
        #[arg(short, long, value_enum, default_value = "auto")]
        format: InputFormat,

        /// Output backend
        #[arg(short, long, value_enum)]
        backend: Backend,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Export all types (add 'export' keyword)
        #[arg(long, default_value = "true")]
        export: bool,

        /// Generate type inference (for Zod/Valibot)
        #[arg(long)]
        infer_types: bool,

        /// Make types readonly/frozen
        #[arg(long)]
        readonly: bool,

        /// Package name (for Go)
        #[arg(long, default_value = "types")]
        package: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum InputFormat {
    /// Auto-detect from file content
    Auto,
    /// JSON Schema
    JsonSchema,
    /// OpenAPI 3.x
    OpenApi,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Backend {
    /// TypeScript interfaces/types
    Typescript,
    /// Zod schemas (TypeScript)
    Zod,
    /// Valibot schemas (TypeScript)
    Valibot,
    /// Python dataclasses
    Python,
    /// Pydantic models
    Pydantic,
    /// Go structs
    Go,
    /// Rust structs with serde
    Rust,
}

/// Run the generate command
pub fn run(args: GenerateArgs) -> i32 {
    match args.target {
        GenerateTarget::Client { spec, lang, output } => run_legacy_client(spec, lang, output),
        GenerateTarget::Types {
            schema,
            name,
            lang,
            output,
        } => run_legacy_types(schema, name, lang, output),
        GenerateTarget::CliSnapshot {
            binary,
            output,
            name,
        } => run_cli_snapshot(binary, output, name),
        GenerateTarget::Typegen {
            input,
            format,
            backend,
            output,
            export,
            infer_types,
            readonly,
            package,
        } => run_typegen(
            input,
            format,
            backend,
            output,
            export,
            infer_types,
            readonly,
            package,
        ),
    }
}

fn run_legacy_client(spec: PathBuf, lang: String, output: Option<PathBuf>) -> i32 {
    let Some(generator) = normalize_openapi::find_generator(&lang) else {
        eprintln!("Unknown language: {}. Available:", lang);
        for (lang, variant) in normalize_openapi::list_generators() {
            eprintln!("  {} ({})", lang, variant);
        }
        return 1;
    };

    let content = match std::fs::read_to_string(&spec) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", spec.display(), e);
            return 1;
        }
    };
    let spec_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return 1;
        }
    };

    let code = generator.generate(&spec_json);

    if let Some(path) = output {
        if let Err(e) = std::fs::write(&path, &code) {
            eprintln!("Failed to write {}: {}", path.display(), e);
            return 1;
        }
        eprintln!("Generated {}", path.display());
    } else {
        print!("{}", code);
    }
    0
}

fn run_legacy_types(schema: PathBuf, name: String, lang: String, output: Option<PathBuf>) -> i32 {
    let Some(generator) = normalize_jsonschema::find_generator(&lang) else {
        eprintln!("Unknown language: {}. Available:", lang);
        for l in normalize_jsonschema::list_generators() {
            eprintln!("  {}", l);
        }
        return 1;
    };

    let content = match std::fs::read_to_string(&schema) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", schema.display(), e);
            return 1;
        }
    };
    let schema_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return 1;
        }
    };

    let code = generator.generate(&schema_json, &name);

    if let Some(path) = output {
        if let Err(e) = std::fs::write(&path, &code) {
            eprintln!("Failed to write {}: {}", path.display(), e);
            return 1;
        }
        eprintln!("Generated {}", path.display());
    } else {
        print!("{}", code);
    }
    0
}

#[allow(clippy::too_many_arguments)]
fn run_typegen(
    input: PathBuf,
    format: InputFormat,
    backend: Backend,
    output: Option<PathBuf>,
    export: bool,
    infer_types: bool,
    readonly: bool,
    package: String,
) -> i32 {
    use normalize_typegen::{
        ir::Schema,
        output::{
            go::{GoOptions, generate_go_types},
            pydantic::{PydanticOptions, generate_pydantic},
            python::{PythonOptions, generate_python_types},
            rust::{RustOptions, generate_rust_types},
            typescript::{TypeScriptOptions, generate_typescript_types},
            valibot::{ValibotOptions, generate_valibot},
            zod::{ZodOptions, generate_zod},
        },
        parse_json_schema, parse_openapi,
    };

    // Read input (file or stdin)
    let content = if input.as_os_str() == "-" {
        use std::io::Read;
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("Failed to read stdin: {}", e);
            return 1;
        }
        buf
    } else {
        match std::fs::read_to_string(&input) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read {}: {}", input.display(), e);
                return 1;
            }
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return 1;
        }
    };

    // Detect format if auto
    let detected_format = match format {
        InputFormat::Auto => {
            if json.get("openapi").is_some() {
                InputFormat::OpenApi
            } else {
                InputFormat::JsonSchema
            }
        }
        f => f,
    };

    // Parse to IR
    let schema: Schema = match detected_format {
        InputFormat::JsonSchema | InputFormat::Auto => match parse_json_schema(&json) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse JSON Schema: {}", e);
                return 1;
            }
        },
        InputFormat::OpenApi => match parse_openapi(&json) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse OpenAPI: {}", e);
                return 1;
            }
        },
    };

    // Generate code
    let code = match backend {
        Backend::Typescript => generate_typescript_types(
            &schema,
            &TypeScriptOptions {
                export,
                readonly,
                ..Default::default()
            },
        ),
        Backend::Zod => generate_zod(
            &schema,
            &ZodOptions {
                export,
                infer_types,
            },
        ),
        Backend::Valibot => generate_valibot(
            &schema,
            &ValibotOptions {
                export,
                infer_types,
            },
        ),
        Backend::Python => generate_python_types(
            &schema,
            &PythonOptions {
                frozen: readonly,
                ..Default::default()
            },
        ),
        Backend::Pydantic => generate_pydantic(
            &schema,
            &PydanticOptions {
                frozen: readonly,
                ..Default::default()
            },
        ),
        Backend::Go => generate_go_types(&schema, &GoOptions::with_package(package)),
        Backend::Rust => {
            if readonly {
                // For Rust, readonly doesn't make sense, but we can skip serde
                generate_rust_types(
                    &schema,
                    &RustOptions {
                        debug: true,
                        clone: true,
                        public: true,
                        ..Default::default()
                    },
                )
            } else {
                generate_rust_types(&schema, &RustOptions::with_serde())
            }
        }
    };

    // Output
    if let Some(path) = output {
        if let Err(e) = std::fs::write(&path, &code) {
            eprintln!("Failed to write {}: {}", path.display(), e);
            return 1;
        }
        eprintln!("Generated {}", path.display());
    } else {
        print!("{}", code);
    }

    0
}

fn run_cli_snapshot(binary: PathBuf, output: Option<PathBuf>, name: Option<String>) -> i32 {
    use std::collections::HashSet;
    use std::process::Command;

    let bin_name = name.unwrap_or_else(|| {
        binary
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "cli".to_string())
    });

    // Verify binary exists and runs
    if let Err(e) = Command::new(&binary).arg("--help").output() {
        eprintln!("Failed to run {}: {}", binary.display(), e);
        return 1;
    }

    // Recursively discover all commands using moss-cli-parser
    fn discover_commands(
        binary: &std::path::Path,
        prefix: &[String],
        visited: &mut HashSet<String>,
    ) -> Vec<Vec<String>> {
        let key = prefix.join(" ");
        if visited.contains(&key) {
            return vec![];
        }
        visited.insert(key);

        let mut result = vec![prefix.to_vec()];

        // Get help for this command
        let mut cmd = Command::new(binary);
        for arg in prefix {
            cmd.arg(arg);
        }
        cmd.arg("--help");

        let help = match cmd.output() {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(_) => return result,
        };

        // Parse help output using moss-cli-parser
        let spec = match normalize_cli_parser::parse_help(&help) {
            Ok(s) => s,
            Err(_) => return result,
        };

        // Recurse into subcommands
        for subcmd in spec.commands {
            let mut new_prefix = prefix.to_vec();
            new_prefix.push(subcmd.name);
            result.extend(discover_commands(binary, &new_prefix, visited));
        }

        result
    }

    let mut visited = HashSet::new();
    let commands = discover_commands(&binary, &[], &mut visited);

    eprintln!("Discovered {} commands", commands.len());

    // Generate test file
    let mut code = String::new();
    code.push_str(&format!(
        r#"//! CLI snapshot tests for {} - verify --help output doesn't change unexpectedly.
//!
//! These tests ensure CLI breaking changes are detected during review.
//! Run `cargo insta review` to update snapshots after intentional changes.

use assert_cmd::Command;

fn {}() -> Command {{
    Command::cargo_bin("{}").unwrap()
}}

fn snapshot_help(args: &[&str]) -> String {{
    let mut cmd = {}();
    for arg in args {{
        cmd.arg(arg);
    }}
    cmd.arg("--help");

    let output = cmd.output().expect("failed to execute");
    String::from_utf8_lossy(&output.stdout).to_string()
}}

"#,
        bin_name, bin_name, bin_name, bin_name
    ));

    // Generate test for each command
    for cmd_path in &commands {
        let test_name = if cmd_path.is_empty() {
            "root".to_string()
        } else {
            cmd_path.join("_").replace('-', "_")
        };

        let args_str = if cmd_path.is_empty() {
            "&[]".to_string()
        } else {
            format!(
                "&[{}]",
                cmd_path
                    .iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        code.push_str(&format!(
            r#"#[test]
fn test_help_{}() {{
    insta::assert_snapshot!(snapshot_help({}));
}}

"#,
            test_name, args_str
        ));
    }

    // Output
    if let Some(path) = output {
        if let Err(e) = std::fs::write(&path, &code) {
            eprintln!("Failed to write {}: {}", path.display(), e);
            return 1;
        }
        eprintln!("Generated {} ({} tests)", path.display(), commands.len());
    } else {
        print!("{}", code);
    }

    0
}
