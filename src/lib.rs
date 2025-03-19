#[macro_use]
extern crate serde;

// Define version constant
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Constants module
pub mod constants {
    pub const LATEST_PROTOCOL_VERSION: &str = "0.1.0";
    pub const JSONRPC_VERSION: &str = "2.0";
}

// Export all modules
pub mod auth;
pub mod client;
pub mod error;
pub mod macros;
pub mod prompt;
pub mod resource;
pub mod schema;
pub mod server;
pub mod tools;
pub mod transport;

// Re-export common macros
#[cfg(feature = "macros")]
pub use mcpr_macros::{
    mcp_auth, mcp_client, mcp_prompt, mcp_resource, mcp_server, mcp_state, mcp_tools,
    mcp_transport, prompt, resource, tool, tool_call, tool_def,
};

// Re-export client module and types
pub use client::Client;
pub use client::ClientInterface;

#[cfg(test)]
mod tests {
    #[cfg(feature = "macros")]
    #[test]
    fn run_macro_tests() {
        // This test simply ensures that the macro crate's tests are run as part of the main crate's test suite
        // The actual tests are in the mcpr-macros crate
    }
}
