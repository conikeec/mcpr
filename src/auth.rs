use crate::error::MCPError;

/// Trait for authentication providers
pub trait AuthProvider {
    /// Authenticate a request with the given token
    fn authenticate(&self, token: &str) -> Result<bool, MCPError>;
}
