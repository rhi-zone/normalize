//! LLM client for workflow engine.
//!
//! Supports all providers from rig: anthropic, openai, google, cohere, groq, etc.

#[cfg(feature = "llm")]
use rig::{
    client::{CompletionClient, ProviderClient},
    completion::Prompt,
    providers,
};

/// Supported LLM providers.
#[cfg(feature = "llm")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Azure,
    Gemini,
    Cohere,
    DeepSeek,
    Groq,
    Mistral,
    Ollama,
    OpenRouter,
    Perplexity,
    Together,
    XAI,
}

#[cfg(feature = "llm")]
impl Provider {
    /// Parse provider from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Some(Self::Anthropic),
            "openai" | "gpt" | "chatgpt" => Some(Self::OpenAI),
            "azure" | "azure-openai" => Some(Self::Azure),
            "google" | "gemini" => Some(Self::Gemini),
            "cohere" => Some(Self::Cohere),
            "deepseek" => Some(Self::DeepSeek),
            "groq" => Some(Self::Groq),
            "mistral" => Some(Self::Mistral),
            "ollama" => Some(Self::Ollama),
            "openrouter" => Some(Self::OpenRouter),
            "perplexity" | "pplx" => Some(Self::Perplexity),
            "together" | "together-ai" => Some(Self::Together),
            "xai" | "grok" => Some(Self::XAI),
            _ => None,
        }
    }

    /// Get default model for this provider.
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-4-20250514",
            Self::OpenAI => "gpt-4o",
            Self::Azure => "gpt-4o",
            Self::Gemini => "gemini-2.0-flash",
            Self::Cohere => "command-r-plus",
            Self::DeepSeek => "deepseek-chat",
            Self::Groq => "llama-3.3-70b-versatile",
            Self::Mistral => "mistral-large-latest",
            Self::Ollama => "llama3.2",
            Self::OpenRouter => "anthropic/claude-3.5-sonnet",
            Self::Perplexity => "llama-3.1-sonar-large-128k-online",
            Self::Together => "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
            Self::XAI => "grok-2-latest",
        }
    }

    /// Get environment variable name for API key.
    pub fn env_var(&self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Azure => "AZURE_OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Cohere => "COHERE_API_KEY",
            Self::DeepSeek => "DEEPSEEK_API_KEY",
            Self::Groq => "GROQ_API_KEY",
            Self::Mistral => "MISTRAL_API_KEY",
            Self::Ollama => "OLLAMA_API_KEY",
            Self::OpenRouter => "OPENROUTER_API_KEY",
            Self::Perplexity => "PERPLEXITY_API_KEY",
            Self::Together => "TOGETHER_API_KEY",
            Self::XAI => "XAI_API_KEY",
        }
    }

    /// List all providers.
    pub fn all() -> &'static [Self] {
        &[
            Self::Anthropic,
            Self::OpenAI,
            Self::Azure,
            Self::Gemini,
            Self::Cohere,
            Self::DeepSeek,
            Self::Groq,
            Self::Mistral,
            Self::Ollama,
            Self::OpenRouter,
            Self::Perplexity,
            Self::Together,
            Self::XAI,
        ]
    }
}

/// LLM client.
#[cfg(feature = "llm")]
pub struct LlmClient {
    provider: Provider,
    model: String,
}

#[cfg(feature = "llm")]
impl LlmClient {
    /// Create a new LLM client.
    pub fn new(provider_str: &str, model: Option<&str>) -> Result<Self, String> {
        let provider = Provider::from_str(provider_str).ok_or_else(|| {
            format!(
                "Unsupported provider: {}. Available: {}",
                provider_str,
                Provider::all()
                    .iter()
                    .map(|p| format!("{:?}", p).to_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        // Check for API key (ollama is optional since it can be local)
        if provider != Provider::Ollama && std::env::var(provider.env_var()).is_err() {
            return Err(format!(
                "Missing {} environment variable for {} provider",
                provider.env_var(),
                provider_str
            ));
        }

        let model = model
            .map(|m| m.to_string())
            .unwrap_or_else(|| provider.default_model().to_string());

        Ok(Self { provider, model })
    }

    /// Generate a completion.
    pub fn complete(&self, system: Option<&str>, prompt: &str) -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(self.complete_async(system, prompt))
    }

    async fn complete_async(&self, system: Option<&str>, prompt: &str) -> Result<String, String> {
        macro_rules! run_provider {
            ($client:expr) => {{
                let client = $client;
                let mut builder = client.agent(&self.model);
                if let Some(sys) = system {
                    builder = builder.preamble(sys);
                }
                let agent = builder.build();
                agent
                    .prompt(prompt)
                    .await
                    .map_err(|e| format!("LLM request failed: {}", e))
            }};
        }

        match self.provider {
            Provider::Anthropic => run_provider!(providers::anthropic::Client::from_env()),
            Provider::OpenAI => run_provider!(providers::openai::Client::from_env()),
            Provider::Azure => run_provider!(providers::azure::Client::from_env()),
            Provider::Gemini => run_provider!(providers::gemini::Client::from_env()),
            Provider::Cohere => run_provider!(providers::cohere::Client::from_env()),
            Provider::DeepSeek => run_provider!(providers::deepseek::Client::from_env()),
            Provider::Groq => run_provider!(providers::groq::Client::from_env()),
            Provider::Mistral => run_provider!(providers::mistral::Client::from_env()),
            Provider::Ollama => run_provider!(providers::ollama::Client::from_env()),
            Provider::OpenRouter => run_provider!(providers::openrouter::Client::from_env()),
            Provider::Perplexity => run_provider!(providers::perplexity::Client::from_env()),
            Provider::Together => run_provider!(providers::together::Client::from_env()),
            Provider::XAI => run_provider!(providers::xai::Client::from_env()),
        }
    }
}

/// Agent response with optional command to execute.
#[derive(Debug)]
pub enum AgentAction {
    /// Execute a command and continue.
    Command { name: String, args: Vec<String> },
    /// Agent is done.
    Done { message: String },
}

/// Parse agent response to extract commands.
/// Format: Lines starting with "> " are commands.
/// "DONE" on its own line means agent is finished.
pub fn parse_agent_response(response: &str) -> AgentAction {
    for line in response.lines() {
        let line = line.trim();

        // Check for done signal
        if line.eq_ignore_ascii_case("done") || line.eq_ignore_ascii_case("[done]") {
            return AgentAction::Done {
                message: response.to_string(),
            };
        }

        // Check for command (lines starting with "> ")
        if let Some(cmd) = line.strip_prefix("> ") {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if let Some((name, args)) = parts.split_first() {
                return AgentAction::Command {
                    name: name.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                };
            }
        }
    }

    // No command found - treat as done with explanation
    AgentAction::Done {
        message: response.to_string(),
    }
}

/// System prompt for agent mode.
pub const AGENT_SYSTEM_PROMPT: &str = r#"Tools:
view <path|symbol|path/symbol>
edit <path|symbol|path/symbol> <task>
analyze [--health|--complexity] [path]
grep <pattern> [path]
lint [--fix] [path]
shell <command>

Run: "> cmd". End: DONE"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        let response = "Let me check.\n> view src/main.rs\n";
        match parse_agent_response(response) {
            AgentAction::Command { name, args } => {
                assert_eq!(name, "view");
                assert_eq!(args, vec!["src/main.rs"]);
            }
            _ => panic!("Expected Command"),
        }
    }

    #[test]
    fn test_parse_done() {
        let response = "I found the issue.\nDONE";
        match parse_agent_response(response) {
            AgentAction::Done { .. } => {}
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_parse_no_command() {
        let response = "Just explaining something.";
        match parse_agent_response(response) {
            AgentAction::Done { .. } => {}
            _ => panic!("Expected Done for no-command response"),
        }
    }

    #[cfg(feature = "llm")]
    #[test]
    fn test_provider_parsing() {
        assert_eq!(Provider::from_str("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("claude"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("openai"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("gpt"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("google"), Some(Provider::Gemini));
        assert_eq!(Provider::from_str("gemini"), Some(Provider::Gemini));
        assert_eq!(Provider::from_str("groq"), Some(Provider::Groq));
        assert_eq!(Provider::from_str("ollama"), Some(Provider::Ollama));
        assert_eq!(Provider::from_str("unknown"), None);
    }

    #[cfg(feature = "llm")]
    #[test]
    fn test_all_providers_have_defaults() {
        for provider in Provider::all() {
            assert!(!provider.default_model().is_empty());
            assert!(!provider.env_var().is_empty());
        }
    }
}
