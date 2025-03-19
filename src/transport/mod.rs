//! Transport layer for MCP communication
//!
//! This module provides transport implementations for the Model Context Protocol (MCP).
//! Transports handle the underlying mechanics of how messages are sent and received.
//!
//! The following transport types are supported:
//! - Stdio: Standard input/output for local processes
//! - SSE: Server-Sent Events for server-to-client messages with HTTP POST for client-to-server
//!
//! The following transport types are planned but not yet implemented:
//! - WebSocket: Bidirectional communication over WebSockets (TBD)
//!
//! Note: There are some linter errors related to async/await in this file.
//! These errors occur because the async implementations require proper async
//! HTTP and WebSocket clients. To fix these errors, you would need to:
//! 1. Add proper async dependencies to your Cargo.toml
//! 2. Implement the async methods using those dependencies
//! 3. Use proper async/await syntax throughout the implementation
//!
//! For now, the synchronous implementations are provided and work correctly.

use crate::error::MCPError;
use serde::{de::DeserializeOwned, Serialize};

/// Type alias for a closure that is called when an error occurs
pub type ErrorCallback = Box<dyn Fn(&MCPError) + Send + Sync>;

/// Type alias for a closure that is called when a message is received
pub type MessageCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Type alias for a closure that is called when the connection is closed
pub type CloseCallback = Box<dyn Fn() + Send + Sync>;

/// Transport trait for MCP communication
pub trait Transport {
    /// Start processing messages
    fn start(&mut self) -> Result<(), MCPError>;

    /// Send a message as a JSON string
    fn send_json(&mut self, json_string: &str) -> Result<(), MCPError>;

    /// Receive a message as a JSON string
    fn receive_json(&mut self) -> Result<String, MCPError>;

    /// Close the connection
    fn close(&mut self) -> Result<(), MCPError>;

    /// Check if the transport is connected
    fn is_connected(&self) -> bool;

    /// Set callback for when a message is received
    fn on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>);

    /// Set callback for when an error occurs
    fn on_error(&mut self, callback: Box<dyn Fn(&MCPError) + Send + Sync>);

    /// Set callback for when the connection is closed
    fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>);

    /// Set callback for when the connection is closed (deprecated, use on_close)
    fn set_on_close(&mut self, callback: Option<CloseCallback>);

    /// Set callback for when an error occurs (deprecated, use on_error)
    fn set_on_error(&mut self, callback: Option<ErrorCallback>);
}

/// Extension trait with convenience methods using generic parameters
pub trait TransportExt: Transport {
    /// Send a message using serialization
    fn send<T: Serialize>(&mut self, message: &T) -> Result<(), MCPError> {
        let json = serde_json::to_string(message)?;
        self.send_json(&json)
    }

    /// Receive a message and deserialize it
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, MCPError> {
        let json = self.receive_json()?;
        match serde_json::from_str(&json) {
            Ok(value) => Ok(value),
            Err(e) => Err(MCPError::Deserialization(e)),
        }
    }

    /// Set callback for when a message is received (with generic type parameter)
    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        if let Some(cb) = callback {
            self.on_message(Box::new(cb));
        }
    }
}

// Implement TransportExt for all types that implement Transport
impl<T: Transport> TransportExt for T {}

/// Standard IO transport
pub mod stdio;

/// Server-Sent Events (SSE) transport
pub mod sse;

// Note: WebSocket transport is planned but not yet implemented
