use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum MCPError {
    #[error("JSON serialization error: {0}")]
    Serialization(String),

    #[error("JSON deserialization error: {0}")]
    Deserialization(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

impl From<serde_json::Error> for MCPError {
    fn from(err: serde_json::Error) -> Self {
        MCPError::Serialization(err.to_string())
    }
}
