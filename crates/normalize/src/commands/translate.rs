//! Translate command - language enums used by the service layer.

/// Source language for translation.
#[derive(Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    /// TypeScript/JavaScript
    Typescript,
    /// Lua
    Lua,
    /// Python
    Python,
}

/// Target language for translation.
#[derive(Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
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
