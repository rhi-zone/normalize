//! Translate command - convert code between programming languages.

use clap::{Args, ValueEnum};
use std::path::PathBuf;

/// Translate command arguments
#[derive(Args, serde::Deserialize, schemars::JsonSchema)]
pub struct TranslateArgs {
    /// Input source file, use - for stdin
    pub input: PathBuf,

    /// Source language (required when using stdin, auto-detect from extension otherwise)
    #[arg(short, long)]
    pub from: Option<SourceLanguage>,

    /// Target language
    #[arg(short, long)]
    pub to: TargetLanguage,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    /// TypeScript/JavaScript
    Typescript,
    /// Lua
    Lua,
    /// Python
    Python,
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TargetLanguage {
    /// TypeScript
    Typescript,
    /// Lua
    Lua,
    /// Python
    Python,
}

impl SourceLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceLanguage::Typescript => "typescript",
            SourceLanguage::Lua => "lua",
            SourceLanguage::Python => "python",
        }
    }
}

impl std::fmt::Display for SourceLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SourceLanguage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "typescript" => Ok(Self::Typescript),
            "lua" => Ok(Self::Lua),
            "python" => Ok(Self::Python),
            _ => Err(format!("unknown source language: {s}")),
        }
    }
}

impl TargetLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetLanguage::Typescript => "typescript",
            TargetLanguage::Lua => "lua",
            TargetLanguage::Python => "python",
        }
    }
}

impl std::fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TargetLanguage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "typescript" => Ok(Self::Typescript),
            "lua" => Ok(Self::Lua),
            "python" => Ok(Self::Python),
            _ => Err(format!("unknown target language: {s}")),
        }
    }
}

/// Service-callable translate command.
pub fn cmd_translate_service(
    input: &str,
    from: Option<SourceLanguage>,
    to: TargetLanguage,
    output: Option<&str>,
) -> Result<crate::service::TranslateResult, String> {
    let is_stdin = input == "-";
    let input_path = std::path::PathBuf::from(input);

    // Read input (file or stdin)
    let content = if is_stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        buf
    } else {
        std::fs::read_to_string(&input_path)
            .map_err(|e| format!("Failed to read {}: {}", input, e))?
    };

    // Determine source language
    let source_lang = match from {
        Some(lang) => lang.as_str(),
        None => {
            if is_stdin {
                return Err("--from is required when reading from stdin".to_string());
            }
            match input_path.extension().and_then(|e| e.to_str()) {
                Some("ts") | Some("tsx") | Some("js") | Some("jsx") => "typescript",
                Some("lua") => "lua",
                Some("py") => "python",
                _ => {
                    return Err(
                        "Cannot detect language from extension. Use --from to specify source language."
                            .to_string(),
                    );
                }
            }
        }
    };

    let reader = normalize_surface_syntax::registry::reader_for_language(source_lang)
        .ok_or_else(|| format!("No reader available for language: {}", source_lang))?;

    let target_lang = to.as_str();
    let writer = normalize_surface_syntax::registry::writer_for_language(target_lang)
        .ok_or_else(|| format!("No writer available for language: {}", target_lang))?;

    let ir = reader
        .read(&content)
        .map_err(|e| format!("Failed to parse {} as {}: {}", input, source_lang, e))?;

    let code = writer.write(&ir);

    if let Some(path) = output {
        std::fs::write(path, &code).map_err(|e| format!("Failed to write {}: {}", path, e))?;
        eprintln!("Translated {} -> {} ({})", input, path, target_lang);
        Ok(crate::service::TranslateResult {
            code,
            source_language: source_lang.to_string(),
            target_language: target_lang.to_string(),
            input_path: input.to_string(),
            output_path: Some(path.to_string()),
        })
    } else {
        Ok(crate::service::TranslateResult {
            code,
            source_language: source_lang.to_string(),
            target_language: target_lang.to_string(),
            input_path: input.to_string(),
            output_path: None,
        })
    }
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(TranslateArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run the translate command
pub fn run(args: TranslateArgs, input_schema: bool, params_json: Option<&str>) -> i32 {
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
    let is_stdin = args.input.as_os_str() == "-";

    // Read input (file or stdin)
    let content = if is_stdin {
        use std::io::Read;
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("Failed to read stdin: {}", e);
            return 1;
        }
        buf
    } else {
        match std::fs::read_to_string(&args.input) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read {}: {}", args.input.display(), e);
                return 1;
            }
        }
    };

    // Determine source language
    let source_lang = match args.from {
        Some(lang) => lang.as_str(),
        None => {
            if is_stdin {
                eprintln!("--from is required when reading from stdin");
                return 1;
            }
            // Auto-detect from extension
            match args.input.extension().and_then(|e| e.to_str()) {
                Some("ts") | Some("tsx") | Some("js") | Some("jsx") => "typescript",
                Some("lua") => "lua",
                Some("py") => "python",
                _ => {
                    eprintln!(
                        "Cannot detect language from extension. Use --from to specify source language."
                    );
                    return 1;
                }
            }
        }
    };

    // Get reader
    let reader = match normalize_surface_syntax::registry::reader_for_language(source_lang) {
        Some(r) => r,
        None => {
            eprintln!("No reader available for language: {}", source_lang);
            eprintln!("Available readers:");
            for r in normalize_surface_syntax::registry::readers() {
                eprintln!("  {} ({})", r.language(), r.extensions().join(", "));
            }
            return 1;
        }
    };

    // Get writer
    let target_lang = args.to.as_str();
    let writer = match normalize_surface_syntax::registry::writer_for_language(target_lang) {
        Some(w) => w,
        None => {
            eprintln!("No writer available for language: {}", target_lang);
            eprintln!("Available writers:");
            for w in normalize_surface_syntax::registry::writers() {
                eprintln!("  {} (.{})", w.language(), w.extension());
            }
            return 1;
        }
    };

    // Parse source
    let ir = match reader.read(&content) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!(
                "Failed to parse {} as {}: {}",
                args.input.display(),
                source_lang,
                e
            );
            return 1;
        }
    };

    // Generate output
    let output = writer.write(&ir);

    // Write output
    if let Some(path) = args.output {
        if let Err(e) = std::fs::write(&path, &output) {
            eprintln!("Failed to write {}: {}", path.display(), e);
            return 1;
        }
        eprintln!(
            "Translated {} -> {} ({})",
            args.input.display(),
            path.display(),
            target_lang
        );
    } else {
        print!("{}", output);
    }

    0
}
