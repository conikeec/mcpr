use crate::error::MCPError;
use crate::schema::common::Tool;
use serde_json::Value;

/// Trait for providing tools
pub trait ToolsProvider {
    /// Get a list of available tools
    fn get_tools(&self) -> Vec<Tool>;

    /// Execute a tool with the given parameters
    fn execute_tool(&self, name: &str, params: &Value) -> Result<Value, MCPError>;
}

/// Struct for tool parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolParameters {
    /// Tool name
    pub name: String,

    /// Tool parameters as JSON
    pub parameters: Value,
}

/// Struct for tool execution results
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// Tool execution result as JSON
    pub result: Value,
}
