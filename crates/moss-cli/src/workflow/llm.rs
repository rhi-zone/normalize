//! LLM strategy for workflow execution.
//!
//! This module is only compiled when the "llm" feature is enabled.

#[cfg(feature = "llm")]
use rig_core::{completion::Prompt, providers};

/// LLM strategy trait for workflow execution.
pub trait LlmStrategy: Send + Sync {
    /// Generate a completion from a prompt.
    fn complete(&self, prompt: &str) -> Result<String, String>;

    /// Generate with system prompt.
    fn complete_with_system(&self, system: &str, prompt: &str) -> Result<String, String>;
}

/// No LLM - for workflows that don't need it.
pub struct NoLlm;

impl LlmStrategy for NoLlm {
    fn complete(&self, _prompt: &str) -> Result<String, String> {
        Err("LLM not configured for this workflow".to_string())
    }

    fn complete_with_system(&self, _system: &str, _prompt: &str) -> Result<String, String> {
        Err("LLM not configured for this workflow".to_string())
    }
}

#[cfg(feature = "llm")]
pub struct RigLlm {
    model: String,
    provider: String,
}

#[cfg(feature = "llm")]
impl RigLlm {
    pub fn new(provider: &str, model: Option<&str>) -> Result<Self, String> {
        let model = model
            .map(|m| m.to_string())
            .unwrap_or_else(|| default_model(provider));

        Ok(Self {
            model,
            provider: provider.to_string(),
        })
    }

    fn get_client(&self) -> Result<rig_core::providers::anthropic::Client, String> {
        match self.provider.as_str() {
            "anthropic" => Ok(providers::anthropic::Client::from_env()),
            _ => Err(format!("Unsupported provider: {}", self.provider)),
        }
    }
}

#[cfg(feature = "llm")]
fn default_model(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514".to_string(),
        "openai" => "gpt-4o".to_string(),
        "google" => "gemini-2.0-flash".to_string(),
        _ => "claude-sonnet-4-20250514".to_string(),
    }
}

#[cfg(feature = "llm")]
impl LlmStrategy for RigLlm {
    fn complete(&self, prompt: &str) -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        rt.block_on(async {
            let client = self.get_client()?;
            let agent = client.agent(&self.model).build();

            agent
                .prompt(prompt)
                .await
                .map_err(|e| format!("LLM request failed: {}", e))
        })
    }

    fn complete_with_system(&self, system: &str, prompt: &str) -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        rt.block_on(async {
            let client = self.get_client()?;
            let agent = client.agent(&self.model).preamble(system).build();

            agent
                .prompt(prompt)
                .await
                .map_err(|e| format!("LLM request failed: {}", e))
        })
    }
}

/// Build an LLM strategy from workflow config.
pub fn build_llm_strategy(
    _provider: Option<&str>,
    _model: Option<&str>,
) -> Box<dyn LlmStrategy> {
    #[cfg(feature = "llm")]
    {
        if let Some(provider) = _provider {
            match RigLlm::new(provider, _model) {
                Ok(llm) => return Box::new(llm),
                Err(e) => {
                    eprintln!("Warning: Failed to initialize LLM: {}", e);
                }
            }
        }
    }

    Box::new(NoLlm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_llm() {
        let llm = NoLlm;
        assert!(llm.complete("test").is_err());
    }

    #[test]
    fn test_build_llm_strategy_without_provider() {
        let strategy = build_llm_strategy(None, None);
        assert!(strategy.complete("test").is_err());
    }
}
