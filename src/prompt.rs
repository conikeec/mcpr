use crate::error::MCPError;
use crate::schema::common::{Prompt, PromptMessage};

/// Trait for providing prompt templates and messages
pub trait PromptProvider {
    /// Get a list of available prompts
    fn get_prompts(&self) -> Vec<Prompt>;

    /// Get the messages for a specific prompt
    fn get_prompt_messages(&self, name: &str) -> Result<Vec<PromptMessage>, MCPError>;
}
