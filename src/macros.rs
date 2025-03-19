//! Procedural macros for simplified MCP implementations
//!
//! This module re-exports the procedural macros from the mcpr-macros crate.
//! These macros help reduce boilerplate when implementing MCP components.
//!
//! ## Example
//!
//! ```rust,ignore
//! use mcpr::macros::mcp_transport;
//! use mcpr::error::MCPError;
//!
//! #[mcp_transport]
//! struct MyTransport {
//!     is_connected: bool,
//!     on_close: Option<Box<dyn Fn() + Send + Sync>>,
//!     on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
//!     on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
//!     // Transport-specific fields
//! }
//! ```

#[cfg(feature = "macros")]
pub use mcpr_macros::*;

#[cfg(not(feature = "macros"))]
mod placeholder {
    // This ensures the module exists even without the macros feature
}
