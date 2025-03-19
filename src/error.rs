use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Serialize)]
pub enum MCPError {
    Transport(String),
    Serialization(#[serde(skip)] serde_json::Error),
    Deserialization(#[serde(skip)] serde_json::Error),
    Protocol(String),
    NotFound(String),
    InvalidRequest(String),

    // Authentication and authorization errors
    Authentication(String),
    Authorization(String),

    // State machine errors
    State(String),
    Transition(String),

    // Transport connection errors
    AlreadyConnected,
    NotConnected,

    // Timeout error
    Timeout,

    // Internal error
    Internal(String),
}

// Custom implementation for Deserialize that doesn't require serde_json::Error to implement Default
impl<'de> Deserialize<'de> for MCPError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Transport,
            Protocol,
            NotFound,
            InvalidRequest,
            Authentication,
            Authorization,
            State,
            Transition,
            AlreadyConnected,
            NotConnected,
            Timeout,
            Internal,
        }

        #[derive(Deserialize)]
        struct ErrorHelper {
            #[serde(rename = "type")]
            field: Field,
            message: Option<String>,
        }

        let helper = ErrorHelper::deserialize(deserializer)?;

        match helper.field {
            Field::Transport => Ok(MCPError::Transport(helper.message.unwrap_or_default())),
            Field::Protocol => Ok(MCPError::Protocol(helper.message.unwrap_or_default())),
            Field::NotFound => Ok(MCPError::NotFound(helper.message.unwrap_or_default())),
            Field::InvalidRequest => {
                Ok(MCPError::InvalidRequest(helper.message.unwrap_or_default()))
            }
            Field::Authentication => {
                Ok(MCPError::Authentication(helper.message.unwrap_or_default()))
            }
            Field::Authorization => Ok(MCPError::Authorization(helper.message.unwrap_or_default())),
            Field::State => Ok(MCPError::State(helper.message.unwrap_or_default())),
            Field::Transition => Ok(MCPError::Transition(helper.message.unwrap_or_default())),
            Field::AlreadyConnected => Ok(MCPError::AlreadyConnected),
            Field::NotConnected => Ok(MCPError::NotConnected),
            Field::Timeout => Ok(MCPError::Timeout),
            Field::Internal => Ok(MCPError::Internal(helper.message.unwrap_or_default())),
        }
    }
}

impl fmt::Display for MCPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MCPError::Transport(msg) => write!(f, "Transport error: {}", msg),
            MCPError::Serialization(err) => write!(f, "Serialization error: {}", err),
            MCPError::Deserialization(err) => write!(f, "Deserialization error: {}", err),
            MCPError::Protocol(msg) => write!(f, "Protocol error: {}", msg),
            MCPError::NotFound(msg) => write!(f, "Not found: {}", msg),
            MCPError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            MCPError::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            MCPError::Authorization(msg) => write!(f, "Authorization error: {}", msg),
            MCPError::State(msg) => write!(f, "State error: {}", msg),
            MCPError::Transition(msg) => write!(f, "Transition error: {}", msg),
            MCPError::AlreadyConnected => write!(f, "Transport error: Already connected"),
            MCPError::NotConnected => write!(f, "Transport error: Not connected"),
            MCPError::Timeout => write!(f, "Transport error: Operation timed out"),
            MCPError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for MCPError {}

// Implement From<serde_json::Error> for MCPError to allow ? operator to work
impl From<serde_json::Error> for MCPError {
    fn from(error: serde_json::Error) -> Self {
        MCPError::Serialization(error)
    }
}

impl MCPError {
    /// Check if an error is fatal and requires the connection to be terminated
    pub fn is_fatal(&self) -> bool {
        match self {
            // These errors are considered fatal for a connection
            MCPError::Transport(_) => true,
            MCPError::Protocol(_) => true,
            MCPError::NotConnected => true,
            MCPError::Authentication(_) => true,
            MCPError::Authorization(_) => true,

            // These errors can be recovered from
            MCPError::Serialization(_) => false,
            MCPError::Deserialization(_) => false,
            MCPError::NotFound(_) => false,
            MCPError::InvalidRequest(_) => false,
            MCPError::State(_) => false,
            MCPError::Transition(_) => false,
            MCPError::AlreadyConnected => false,
            MCPError::Timeout => false,
            MCPError::Internal(_) => false,
        }
    }
}
