//! Transport layer for MCP communication
//!
//! This module provides transport implementations for the Model Context Protocol (MCP).
//! Transports handle the underlying mechanics of how messages are sent and received.
//!
//! The following transport types are supported:
//! - Stdio: Standard input/output for local processes
//! - SSE: Server-Sent Events for server-to-client messages with HTTP POST for client-to-server
//!
//! The transport implementations are now fully async, using tokio for async I/O.

use crate::error::MCPError;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

/// Type alias for a closure that is called when an error occurs
pub type ErrorCallback = Box<dyn Fn(&MCPError) + Send + Sync>;

/// Type alias for a closure that is called when a message is received
pub type MessageCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Type alias for a closure that is called when the connection is closed
pub type CloseCallback = Box<dyn Fn() + Send + Sync>;

/// Transport trait for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Start processing messages
    async fn start(&mut self) -> Result<(), MCPError>;

    /// Send a message
    async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError>;

    /// Receive a message
    async fn receive<T: DeserializeOwned + Send + Sync>(&mut self) -> Result<T, MCPError>;

    /// Close the connection
    async fn close(&mut self) -> Result<(), MCPError>;

    /// Set callback for when the connection is closed
    fn set_on_close(&mut self, callback: Option<CloseCallback>);

    /// Set callback for when an error occurs
    fn set_on_error(&mut self, callback: Option<ErrorCallback>);

    /// Set callback for when a message is received
    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static;
}

/// Standard IO transport
pub mod stdio;

/// Server-Sent Events (SSE) transport
pub mod sse;

// Tests for SSE transport
#[cfg(test)]
mod sse_tests;

// Temporarily comment out this module due to dependency errors
// #[cfg(test)]
// mod sse_tests;
