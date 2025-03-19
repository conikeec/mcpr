use crate::error::MCPError;
use crate::schema::common::{Resource, ResourceContents, ResourceTemplate};
use serde_json::Value;

/// Trait for providing resources
pub trait ResourceProvider {
    /// Get user information
    fn user_info(&self) -> Value;

    /// Get product information
    fn product_info(&self) -> Value;

    /// Get available resources
    fn get_resources(&self) -> Vec<Resource>;

    /// Get available resource templates
    fn get_resource_templates(&self) -> Vec<ResourceTemplate>;

    /// Get a specific resource by URI
    fn get_resource(&self, uri: &str) -> Result<ResourceContents, MCPError>;
}
