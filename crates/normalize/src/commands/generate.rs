//! Generate command - code generation from API specs and schemas.

#[cfg(feature = "cli")]
use std::path::PathBuf;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

/// Service-callable version of run_types.
#[cfg(feature = "cli")]
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
    dry_run: bool,
    split: bool,
) -> Result<crate::service::generate::GenerateReport, String> {
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
            let lang = normalize_languages::support_for_path(&input).map(|s| s.name());
            match lang {
                Some("TypeScript") | Some("TSX") => InputFormat::Typescript,
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

    // Helper: generate code for a schema (one or many types)
    let generate_code = |s: &Schema| -> String {
        match backend {
            Backend::Typescript => generate_typescript_types(
                s,
                &TypeScriptOptions {
                    export,
                    readonly,
                    ..Default::default()
                },
            ),
            Backend::Zod => generate_zod(
                s,
                &ZodOptions {
                    export,
                    infer_types,
                },
            ),
            Backend::Valibot => generate_valibot(
                s,
                &ValibotOptions {
                    export,
                    infer_types,
                },
            ),
            Backend::Python => generate_python_types(
                s,
                &PythonOptions {
                    frozen: readonly,
                    ..Default::default()
                },
            ),
            Backend::Pydantic => generate_pydantic(
                s,
                &PydanticOptions {
                    frozen: readonly,
                    ..Default::default()
                },
            ),
            Backend::Go => generate_go_types(s, &GoOptions::with_package(package.clone())),
            Backend::Rust => {
                if readonly {
                    generate_rust_types(
                        s,
                        &RustOptions {
                            debug: true,
                            clone: true,
                            public: true,
                            ..Default::default()
                        },
                    )
                } else {
                    generate_rust_types(s, &RustOptions::with_serde())
                }
            }
        }
    };

    let extension = match backend {
        Backend::Typescript | Backend::Zod | Backend::Valibot => "ts",
        Backend::Python | Backend::Pydantic => "py",
        Backend::Go => "go",
        Backend::Rust => "rs",
    };

    if split {
        // --split: emit one file per top-level type
        if !dry_run {
            let dir = output.as_deref().ok_or_else(|| {
                "--split requires --output to specify an output directory".to_string()
            })?;
            if dir.is_file() {
                return Err(format!(
                    "--split requires a directory, but {} is a file",
                    dir.display()
                ));
            }
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
        }

        let mut combined_preview = String::new();
        let mut files_written = Vec::new();

        for def in &schema.definitions {
            let mut single = Schema::new();
            single.add(def.clone());
            let code = generate_code(&single);
            let filename = format!("{}.{}", type_name_to_filename(&def.name), extension);

            if dry_run {
                combined_preview.push_str(&format!("--- {} ---\n", filename));
                combined_preview.push_str(&code);
                combined_preview.push('\n');
            } else {
                let dir = output.as_deref().unwrap();
                let path = dir.join(&filename);
                std::fs::write(&path, &code)
                    .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
                eprintln!("Generated {}", path.display());
                files_written.push(path.display().to_string());
            }
        }

        if dry_run {
            Ok(crate::service::generate::GenerateReport {
                output: combined_preview,
                path: None,
            })
        } else {
            Ok(crate::service::generate::GenerateReport {
                output: files_written.join("\n"),
                path: output.map(|p| p.display().to_string()),
            })
        }
    } else {
        // Normal single-file mode
        let code = generate_code(&schema);
        if dry_run {
            let display_path = output
                .as_deref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| format!("output.{}", extension));
            let preview = format!("--- {} ---\n{}", display_path, code);
            Ok(crate::service::generate::GenerateReport {
                output: preview,
                path: None,
            })
        } else {
            write_generate_result(code, output)
        }
    }
}

/// Convert a PascalCase or camelCase type name to snake_case filename.
///
/// Examples: `UserType` → `user_type`, `HTTPSConfig` → `https_config`.
#[cfg(feature = "cli")]
fn type_name_to_filename(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
            if i > 0 && (prev_lower || next_lower) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(feature = "cli")]
fn write_generate_result(
    code: String,
    output: Option<PathBuf>,
) -> Result<crate::service::generate::GenerateReport, String> {
    if let Some(ref path) = output {
        std::fs::write(path, &code)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
        eprintln!("Generated {}", path.display());
        Ok(crate::service::generate::GenerateReport {
            output: code,
            path: Some(path.display().to_string()),
        })
    } else {
        Ok(crate::service::generate::GenerateReport {
            output: code,
            path: None,
        })
    }
}

/// Read input content from file or stdin.
#[cfg(feature = "cli")]
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
