//! Translate command - convert code between programming languages.

use clap::{Args, ValueEnum};
use std::path::PathBuf;

/// Translate command arguments
#[derive(Args)]
pub struct TranslateArgs {
    /// Input source file
    pub input: PathBuf,

    /// Source language (auto-detect from extension if not specified)
    #[arg(short, long)]
    pub from: Option<SourceLanguage>,

    /// Target language
    #[arg(short, long)]
    pub to: TargetLanguage,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum SourceLanguage {
    /// TypeScript/JavaScript
    Typescript,
    /// Lua
    Lua,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TargetLanguage {
    /// TypeScript
    Typescript,
    /// Lua
    Lua,
}

impl SourceLanguage {
    fn as_str(&self) -> &'static str {
        match self {
            SourceLanguage::Typescript => "typescript",
            SourceLanguage::Lua => "lua",
        }
    }
}

impl TargetLanguage {
    fn as_str(&self) -> &'static str {
        match self {
            TargetLanguage::Typescript => "typescript",
            TargetLanguage::Lua => "lua",
        }
    }
}

/// Run the translate command
pub fn run(args: TranslateArgs) -> i32 {
    // Read input file
    let content = match std::fs::read_to_string(&args.input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", args.input.display(), e);
            return 1;
        }
    };

    // Determine source language
    let source_lang = match args.from {
        Some(lang) => lang.as_str(),
        None => {
            // Auto-detect from extension
            match args.input.extension().and_then(|e| e.to_str()) {
                Some("ts") | Some("tsx") | Some("js") | Some("jsx") => "typescript",
                Some("lua") => "lua",
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
