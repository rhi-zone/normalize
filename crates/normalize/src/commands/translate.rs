//! Translate command - convert code between programming languages.

#[derive(Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    /// TypeScript/JavaScript
    Typescript,
    /// Lua
    Lua,
    /// Python
    Python,
}

#[derive(Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
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
#[cfg(feature = "cli")]
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
