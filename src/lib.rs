//! # Model Context Protocol (MCP) for Rust
//!
//! This crate provides a Rust implementation of Anthropic's Model Context Protocol (MCP),
//! an open standard for connecting AI assistants to data sources and tools.
//!
//! The implementation includes:
//! - Schema definitions for MCP messages
//! - Transport layer for communication
//! - High-level client and server implementations
//! - CLI tools for generating server and client stubs
//! - Generator for creating MCP server and client stubs
//!
//! ## High-Level Client
//!
//! The high-level client provides a simple interface for communicating with MCP servers:
//!
//! ```rust,no_run
//! use mcpr::{
//!     client::Client,
//!     transport::stdio::StdioTransport,
//! };
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mcpr::error::MCPError> {
//!     // Create a client with stdio transport
//!     let transport = StdioTransport::new();
//!     let mut client = Client::new(transport);
//!
//!     // Initialize the client
//!     client.initialize().await?;
//!
//!     // Call a tool (example with serde_json::Value)
//!     let request = serde_json::json!({
//!         "param1": "value1",
//!         "param2": "value2"
//!     });
//!     let response: Value = client.call_tool("my_tool", &request).await?;
//!
//!     // Shutdown the client
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## High-Level Server
//!
//! The high-level server makes it easy to create MCP-compatible servers:
//!
//! ```rust,no_run
//! use mcpr::{
//!     error::MCPError,
//!     server::{Server, ServerConfig},
//!     transport::stdio::StdioTransport,
//!     schema::common::Tool,
//! };
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), MCPError> {
//!     // Configure the server
//!     let server_config = ServerConfig::new()
//!         .with_name("My MCP Server")
//!         .with_version("1.0.0")
//!         .with_tool(Tool {
//!             name: "my_tool".to_string(),
//!             description: Some("My awesome tool".to_string()),
//!             input_schema: mcpr::schema::common::ToolInputSchema {
//!                 r#type: "object".to_string(),
//!                 properties: Some([
//!                     ("param1".to_string(), serde_json::json!({
//!                         "type": "string",
//!                         "description": "First parameter"
//!                     })),
//!                     ("param2".to_string(), serde_json::json!({
//!                         "type": "string",
//!                         "description": "Second parameter"
//!                     }))
//!                 ].into_iter().collect()),
//!                 required: Some(vec!["param1".to_string(), "param2".to_string()]),
//!             },
//!         });
//!
//!     // Create the server
//!     let mut server = Server::new(server_config);
//!
//!     // Register tool handlers
//!     server.register_tool_handler("my_tool", |params: Value| async move {
//!         // Parse parameters and handle the tool call
//!         let param1 = params.get("param1")
//!             .and_then(|v| v.as_str())
//!             .ok_or_else(|| MCPError::Protocol("Missing param1".to_string()))?;
//!
//!         let param2 = params.get("param2")
//!             .and_then(|v| v.as_str())
//!             .ok_or_else(|| MCPError::Protocol("Missing param2".to_string()))?;
//!
//!         // Process the parameters and generate a response
//!         let response = serde_json::json!({
//!             "result": format!("Processed {} and {}", param1, param2)
//!         });
//!
//!         Ok(response)
//!     })?;
//!
//!     // Start the server with stdio transport
//!     let transport = StdioTransport::new();
//!     server.serve(transport).await
//! }
//! ```

/// Current version of the MCPR crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod cli;
pub mod client;
pub mod error;
pub mod generator;
pub mod schema;
pub mod server;
pub mod transport;

/// Constants used throughout the library
pub mod constants {
    /// The latest supported MCP protocol version
    pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";
    /// The JSON-RPC version used by MCP
    pub const JSONRPC_VERSION: &str = "2.0";
}
