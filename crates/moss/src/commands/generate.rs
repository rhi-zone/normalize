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
    /// Generate types/validators from schema (new IR-based)
    #[command(name = "codegen")]
    Codegen {
        /// Input schema file (JSON Schema or OpenAPI)
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
        GenerateTarget::Codegen {
            input,
            format,
            backend,
            output,
            export,
            infer_types,
            readonly,
            package,
        } => run_codegen(
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
    let Some(generator) = rhizome_moss_openapi::find_generator(&lang) else {
        eprintln!("Unknown language: {}. Available:", lang);
        for (lang, variant) in rhizome_moss_openapi::list_generators() {
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
    let Some(generator) = rhizome_moss_jsonschema::find_generator(&lang) else {
        eprintln!("Unknown language: {}. Available:", lang);
        for l in rhizome_moss_jsonschema::list_generators() {
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
fn run_codegen(
    input: PathBuf,
    format: InputFormat,
    backend: Backend,
    output: Option<PathBuf>,
    export: bool,
    infer_types: bool,
    readonly: bool,
    package: String,
) -> i32 {
    use rhizome_moss_codegen::{
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

    // Read input file
    let content = match std::fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", input.display(), e);
            return 1;
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
