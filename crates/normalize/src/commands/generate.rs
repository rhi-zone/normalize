//! Generate command - code generation from API specs and schemas.

use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Helper for serde default = true
fn default_true() -> bool {
    true
}

/// Helper for default package name
fn default_package() -> String {
    "types".to_string()
}

/// Generate command arguments
#[derive(Args, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateArgs {
    #[command(subcommand)]
    pub target: GenerateTarget,
}

#[derive(Subcommand, serde::Deserialize, schemars::JsonSchema)]
pub enum GenerateTarget {
    /// Generate API client from OpenAPI spec
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
    /// Generate types/validators from schema
    Types {
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
        #[serde(default = "default_true")]
        export: bool,

        /// Generate type inference (for Zod/Valibot)
        #[arg(long)]
        #[serde(default)]
        infer_types: bool,

        /// Make types readonly/frozen
        #[arg(long)]
        #[serde(default)]
        readonly: bool,

        /// Package name (for Go)
        #[arg(long, default_value = "types")]
        #[serde(default = "default_package")]
        package: String,
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
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InputFormat {
    /// Auto-detect from file content
    Auto,
    /// JSON Schema
    JsonSchema,
    /// OpenAPI 3.x
    OpenApi,
    /// TypeScript source (extract type definitions)
    Typescript,
}

impl std::fmt::Display for InputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::JsonSchema => f.write_str("json-schema"),
            Self::OpenApi => f.write_str("openapi"),
            Self::Typescript => f.write_str("typescript"),
        }
    }
}

impl std::str::FromStr for InputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "json-schema" => Ok(Self::JsonSchema),
            "openapi" => Ok(Self::OpenApi),
            "typescript" => Ok(Self::Typescript),
            _ => Err(format!("unknown input format: {s}")),
        }
    }
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
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

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Typescript => f.write_str("typescript"),
            Self::Zod => f.write_str("zod"),
            Self::Valibot => f.write_str("valibot"),
            Self::Python => f.write_str("python"),
            Self::Pydantic => f.write_str("pydantic"),
            Self::Go => f.write_str("go"),
            Self::Rust => f.write_str("rust"),
        }
    }
}

impl std::str::FromStr for Backend {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "typescript" => Ok(Self::Typescript),
            "zod" => Ok(Self::Zod),
            "valibot" => Ok(Self::Valibot),
            "python" => Ok(Self::Python),
            "pydantic" => Ok(Self::Pydantic),
            "go" => Ok(Self::Go),
            "rust" => Ok(Self::Rust),
            _ => Err(format!("unknown backend: {s}")),
        }
    }
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(GenerateArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run the generate command
pub fn run(args: GenerateArgs, input_schema: bool, params_json: Option<&str>) -> i32 {
    if input_schema {
        print_input_schema();
        return 0;
    }
    // Override args with --params-json if provided
    let args = match params_json {
        Some(json) => match serde_json::from_str(json) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("error: invalid --params-json: {}", e);
                return 1;
            }
        },
        None => args,
    };
    match args.target {
        GenerateTarget::Client { spec, lang, output } => run_client(spec, lang, output),
        GenerateTarget::Types {
            input,
            format,
            backend,
            output,
            export,
            infer_types,
            readonly,
            package,
        } => run_types(
            input,
            format,
            backend,
            output,
            export,
            infer_types,
            readonly,
            package,
        ),
        GenerateTarget::CliSnapshot {
            binary,
            output,
            name,
        } => run_cli_snapshot(binary, output, name),
    }
}

fn run_client(spec: PathBuf, lang: String, output: Option<PathBuf>) -> i32 {
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

/// Service-callable version of run_types.
#[allow(clippy::too_many_arguments)]
pub fn run_types_service(
    input: PathBuf,
    format: InputFormat,
    backend: Backend,
    output: Option<PathBuf>,
    export: bool,
    infer_types: bool,
    readonly: bool,
    package: String,
) -> Result<crate::service::generate::GenerateResult, String> {
    let code = generate_types_code(
        input,
        format,
        backend,
        export,
        infer_types,
        readonly,
        package,
    )?;
    write_generate_result(code, output)
}

/// Service-callable version of run_cli_snapshot.
pub fn run_cli_snapshot_service(
    binary: PathBuf,
    output: Option<PathBuf>,
    name: Option<String>,
) -> Result<crate::service::generate::GenerateResult, String> {
    let code = generate_cli_snapshot_code(&binary, name)?;
    write_generate_result(code, output)
}

fn write_generate_result(
    code: String,
    output: Option<PathBuf>,
) -> Result<crate::service::generate::GenerateResult, String> {
    if let Some(ref path) = output {
        std::fs::write(path, &code)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
        eprintln!("Generated {}", path.display());
        Ok(crate::service::generate::GenerateResult {
            output: code,
            path: Some(path.display().to_string()),
        })
    } else {
        Ok(crate::service::generate::GenerateResult {
            output: code,
            path: None,
        })
    }
}

/// Read input content from file or stdin.
fn read_input(input: &std::path::Path) -> Result<String, String> {
    if input.as_os_str() == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read {}: {}", input.display(), e))
    }
}

/// Core types generation logic (shared by legacy and service paths).
#[allow(clippy::too_many_arguments)]
fn generate_types_code(
    input: PathBuf,
    format: InputFormat,
    backend: Backend,
    export: bool,
    infer_types: bool,
    readonly: bool,
    package: String,
) -> Result<String, String> {
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

    let content = read_input(&input)?;

    let detected_format = match format {
        InputFormat::Auto => {
            let ext = input.extension().and_then(|e| e.to_str());
            match ext {
                Some("ts") | Some("tsx") | Some("d.ts") => InputFormat::Typescript,
                _ => InputFormat::Auto,
            }
        }
        f => f,
    };

    let schema: Schema = if matches!(detected_format, InputFormat::Typescript) {
        normalize_typegen::parse_typescript_types(&content)
            .map_err(|e| format!("Failed to parse TypeScript: {}", e))?
    } else {
        let json: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let json_format = match detected_format {
            InputFormat::Auto => {
                if json.get("openapi").is_some() {
                    InputFormat::OpenApi
                } else {
                    InputFormat::JsonSchema
                }
            }
            f => f,
        };

        match json_format {
            InputFormat::OpenApi => {
                parse_openapi(&json).map_err(|e| format!("Failed to parse OpenAPI: {}", e))?
            }
            _ => parse_json_schema(&json)
                .map_err(|e| format!("Failed to parse JSON Schema: {}", e))?,
        }
    };

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

    Ok(code)
}

/// Core CLI snapshot generation logic.
fn generate_cli_snapshot_code(
    binary: &std::path::Path,
    name: Option<String>,
) -> Result<String, String> {
    use std::collections::HashSet;
    use std::process::Command;

    let bin_name = name.unwrap_or_else(|| {
        binary
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "cli".to_string())
    });

    Command::new(binary)
        .arg("--help")
        .output()
        .map_err(|e| format!("Failed to run {}: {}", binary.display(), e))?;

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

        let mut cmd = Command::new(binary);
        for arg in prefix {
            cmd.arg(arg);
        }
        cmd.arg("--help");

        let help = match cmd.output() {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(_) => return result,
        };

        let spec = match normalize_cli_parser::parse_help(&help) {
            Ok(s) => s,
            Err(_) => return result,
        };

        for subcmd in spec.commands {
            let mut new_prefix = prefix.to_vec();
            new_prefix.push(subcmd.name);
            result.extend(discover_commands(binary, &new_prefix, visited));
        }

        result
    }

    let mut visited = HashSet::new();
    let commands = discover_commands(binary, &[], &mut visited);

    eprintln!("Discovered {} commands", commands.len());

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

    Ok(code)
}

#[allow(clippy::too_many_arguments)]
fn run_types(
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

    // Detect format if auto
    let detected_format = match format {
        InputFormat::Auto => {
            let ext = input.extension().and_then(|e| e.to_str());
            match ext {
                Some("ts") | Some("tsx") | Some("d.ts") => InputFormat::Typescript,
                _ => {
                    // Try to parse as JSON and auto-detect
                    InputFormat::Auto
                }
            }
        }
        f => f,
    };

    // Parse to IR
    let schema: Schema = if matches!(detected_format, InputFormat::Typescript) {
        match normalize_typegen::parse_typescript_types(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse TypeScript: {}", e);
                return 1;
            }
        }
    } else {
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
                return 1;
            }
        };

        let json_format = match detected_format {
            InputFormat::Auto => {
                if json.get("openapi").is_some() {
                    InputFormat::OpenApi
                } else {
                    InputFormat::JsonSchema
                }
            }
            f => f,
        };

        match json_format {
            InputFormat::OpenApi => match parse_openapi(&json) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to parse OpenAPI: {}", e);
                    return 1;
                }
            },
            _ => match parse_json_schema(&json) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to parse JSON Schema: {}", e);
                    return 1;
                }
            },
        }
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

    // Recursively discover all commands using normalize-cli-parser
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

        // Parse help output using normalize-cli-parser
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
