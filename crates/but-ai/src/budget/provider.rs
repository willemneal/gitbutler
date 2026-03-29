//! Provider abstraction shim for LLM backends.
//!
//! Raul's provider interface. The `but-ai` plugin does not introduce a new
//! LLM client -- it uses the existing `but-llm` crate. This module provides
//! the abstraction layer for token counting and provider capabilities.
//!
//! New providers (Gemini, Mistral, local GGUF) are added without recompiling
//! via a **provider shim** protocol: standalone executables named
//! `but-ai-provider-<name>` on PATH, communicating over stdio JSON-RPC.

use crate::types::{ProviderKind, TokenUsage};
use serde::{Deserialize, Serialize};

/// Capabilities reported by a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Provider identifier.
    pub kind: ProviderKind,
    /// Maximum context window size in tokens.
    pub max_context_tokens: u64,
    /// Whether the provider supports tool calling.
    pub supports_tool_calling: bool,
    /// Whether the provider supports structured output (JSON mode).
    pub supports_structured_output: bool,
    /// Whether the provider supports streaming.
    pub supports_streaming: bool,
    /// Cost per 1K input tokens in USD (None if free/local).
    pub cost_per_1k_input: Option<f64>,
    /// Cost per 1K output tokens in USD (None if free/local).
    pub cost_per_1k_output: Option<f64>,
}

/// A provider handle that tracks token usage and capabilities.
pub struct ProviderHandle {
    /// The provider's capabilities.
    capabilities: ProviderCapabilities,
    /// Running token usage for the current session.
    session_usage: TokenUsage,
    /// Running monetary cost for the current session.
    session_cost_usd: f64,
}

impl ProviderHandle {
    /// Create a new provider handle.
    pub fn new(capabilities: ProviderCapabilities) -> Self {
        Self {
            capabilities,
            session_usage: TokenUsage::zero(),
            session_cost_usd: 0.0,
        }
    }

    /// Record token usage from a single LLM call.
    pub fn record_usage(&mut self, input: u64, output: u64) {
        self.session_usage.input += input;
        self.session_usage.output += output;

        // Update cost estimate
        if let Some(input_cost) = self.capabilities.cost_per_1k_input {
            self.session_cost_usd += (input as f64 / 1000.0) * input_cost;
        }
        if let Some(output_cost) = self.capabilities.cost_per_1k_output {
            self.session_cost_usd += (output as f64 / 1000.0) * output_cost;
        }
    }

    /// Get the session's total token usage.
    pub fn session_usage(&self) -> &TokenUsage {
        &self.session_usage
    }

    /// Get the estimated session cost in USD.
    pub fn session_cost_usd(&self) -> f64 {
        self.session_cost_usd
    }

    /// Get the provider's capabilities.
    pub fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    /// Get the provider kind.
    pub fn kind(&self) -> &ProviderKind {
        &self.capabilities.kind
    }

    /// Whether the provider supports tool calling.
    pub fn supports_tool_calling(&self) -> bool {
        self.capabilities.supports_tool_calling
    }

    /// Whether the provider supports structured output.
    pub fn supports_structured_output(&self) -> bool {
        self.capabilities.supports_structured_output
    }

    /// How many tokens remain before hitting the context window limit.
    pub fn context_remaining(&self) -> u64 {
        self.capabilities
            .max_context_tokens
            .saturating_sub(self.session_usage.total())
    }

    /// Reset session tracking (e.g. at the start of a new task).
    pub fn reset_session(&mut self) {
        self.session_usage = TokenUsage::zero();
        self.session_cost_usd = 0.0;
    }
}

/// Well-known provider capability profiles.
pub mod profiles {
    use super::*;

    /// Claude Opus profile (200K context).
    pub fn anthropic_opus() -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::Anthropic,
            max_context_tokens: 200_000,
            supports_tool_calling: true,
            supports_structured_output: true,
            supports_streaming: true,
            cost_per_1k_input: Some(0.015),
            cost_per_1k_output: Some(0.075),
        }
    }

    /// GPT-4o profile (128K context).
    pub fn openai_gpt4o() -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::OpenAI,
            max_context_tokens: 128_000,
            supports_tool_calling: true,
            supports_structured_output: true,
            supports_streaming: true,
            cost_per_1k_input: Some(0.005),
            cost_per_1k_output: Some(0.015),
        }
    }

    /// Ollama local model profile (varies by model, default 8K).
    pub fn ollama_default() -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::Ollama,
            max_context_tokens: 8_000,
            supports_tool_calling: false,
            supports_structured_output: false,
            supports_streaming: true,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
        }
    }

    /// LM Studio local model profile.
    pub fn lmstudio_default() -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::LMStudio,
            max_context_tokens: 32_000,
            supports_tool_calling: true,
            supports_structured_output: false,
            supports_streaming: true,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
        }
    }

    /// External provider shim with user-declared capabilities.
    pub fn shim(name: String, max_context: u64, tool_calling: bool) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::Shim(name),
            max_context_tokens: max_context,
            supports_tool_calling: tool_calling,
            supports_structured_output: false,
            supports_streaming: false,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
        }
    }
}

/// Shim discovery: scan a directory for provider shim executables.
///
/// Provider shims are executables named `but-ai-provider-<name>`.
/// This function returns the names of discovered shims (without the prefix).
///
/// In a real implementation, this would scan PATH. Here it accepts
/// a list of directory entries for testing.
pub fn discover_shims(directory_entries: &[String]) -> Vec<String> {
    let prefix = "but-ai-provider-";
    directory_entries
        .iter()
        .filter_map(|entry| entry.strip_prefix(prefix).map(String::from))
        .collect()
}

/// JSON-RPC method names for the provider shim protocol.
pub mod shim_protocol {
    /// Method to query provider capabilities.
    pub const CAPABILITIES: &str = "capabilities";
    /// Method to send a chat completion request.
    pub const CHAT: &str = "chat";
    /// Method to send a streaming chat completion request.
    pub const STREAM: &str = "stream";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_usage_tracking() {
        let mut handle = ProviderHandle::new(profiles::anthropic_opus());

        handle.record_usage(10_000, 2_000);
        assert_eq!(handle.session_usage().input, 10_000);
        assert_eq!(handle.session_usage().output, 2_000);
        assert!(handle.session_cost_usd() > 0.0);
    }

    #[test]
    fn context_remaining_calculation() {
        let mut handle = ProviderHandle::new(profiles::anthropic_opus());
        assert_eq!(handle.context_remaining(), 200_000);

        handle.record_usage(50_000, 10_000);
        assert_eq!(handle.context_remaining(), 140_000);
    }

    #[test]
    fn local_providers_have_no_cost() {
        let mut handle = ProviderHandle::new(profiles::ollama_default());
        handle.record_usage(5_000, 1_000);
        assert!((handle.session_cost_usd() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn shim_discovery() {
        let entries = vec![
            "but-ai-provider-gemini".to_string(),
            "but-ai-provider-mistral".to_string(),
            "some-other-binary".to_string(),
            "but-ai-provider-local-gguf".to_string(),
        ];
        let shims = discover_shims(&entries);
        assert_eq!(shims, vec!["gemini", "mistral", "local-gguf"]);
    }

    #[test]
    fn session_reset() {
        let mut handle = ProviderHandle::new(profiles::openai_gpt4o());
        handle.record_usage(10_000, 5_000);
        assert!(handle.session_usage().total() > 0);

        handle.reset_session();
        assert_eq!(handle.session_usage().total(), 0);
        assert!((handle.session_cost_usd() - 0.0).abs() < f64::EPSILON);
    }
}
