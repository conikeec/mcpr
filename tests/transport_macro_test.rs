#![cfg(feature = "macros")]

use mcpr::error::MCPError;
use mcpr::macros::mcp_transport;
use mcpr::transport::{CloseCallback, ErrorCallback, Transport};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{self, BufReader, Write};

// Test implementation using the mcp_transport macro
#[mcp_transport]
struct TestStdioTransport {
    is_connected: bool,
    on_close: Option<CloseCallback>,
    on_error: Option<ErrorCallback>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
    reader: BufReader<Box<dyn io::Read + Send>>,
    writer: Box<dyn io::Write + Send>,
}

impl TestStdioTransport {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
            reader: BufReader::new(Box::new(io::empty())),
            writer: Box::new(io::sink()),
        }
    }

    // Override the send method
    fn send<T: Serialize>(&mut self, message: &T) -> Result<(), MCPError> {
        // Custom implementation for tests
        let json = match serde_json::to_string(message) {
            Ok(json) => json,
            Err(e) => return Err(MCPError::Serialization(e)),
        };

        match writeln!(self.writer, "{}", json) {
            Ok(_) => {}
            Err(e) => return Err(MCPError::Transport(format!("Failed to write: {}", e))),
        }

        match self.writer.flush() {
            Ok(_) => Ok(()),
            Err(e) => Err(MCPError::Transport(format!("Failed to flush: {}", e))),
        }
    }

    // Override the receive method
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, MCPError> {
        // Simplified implementation for tests
        Err(MCPError::Transport(
            "Test receive not implemented".to_string(),
        ))
    }
}

#[test]
fn test_transport_start_close() {
    let mut transport = TestStdioTransport::new();

    // Test start
    assert!(!transport.is_connected);
    let result = transport.start();
    assert!(result.is_ok());
    assert!(transport.is_connected);

    // Test close
    let result = transport.close();
    assert!(result.is_ok());
    assert!(!transport.is_connected);
}

#[test]
fn test_transport_callbacks() {
    let mut transport = TestStdioTransport::new();

    // Set callbacks
    let was_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let was_called_clone = was_called.clone();

    transport.set_on_close(Some(Box::new(move || {
        was_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    })));

    // Start and close to trigger callback
    transport.start().unwrap();
    transport.close().unwrap();

    // Verify callback was called
    assert!(was_called.load(std::sync::atomic::Ordering::SeqCst));
}
