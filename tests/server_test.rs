// This is just a trybuild test - it does not need to compile perfectly.
// The purpose of this file is to test macro expansion, not compilation.

// Import the necessary macros for testing
#[cfg(feature = "macros")]
use mcpr::macros::mcp_server;
use std::collections::HashMap;

// Mock error type
struct MCPError;

// Mock Value type for JSON
struct JSONValue;

// Mock Tool type
struct Tool {
    name: String,
    description: Option<String>,
    input_schema: ToolInputSchema,
}

struct ToolInputSchema {
    r#type: String,
    properties: Option<HashMap<String, JSONValue>>,
    required: Option<Vec<String>>,
}

// Define the Result type with both type parameters
type Result<T, E = MCPError> = std::result::Result<T, E>;

// Define the trait
trait McpServer {
    fn tools_list(&self) -> Result<Vec<Tool>>;
}

// Define a simple server
struct MockServer;

// This is here to allow the macro to expand without errors
mod mcpr {
    pub mod schema {
        pub mod common {
            use crate::JSONValue;
            use std::collections::HashMap;

            pub struct Tool {
                pub name: String,
                pub description: Option<String>,
                pub input_schema: ToolInputSchema,
            }

            pub struct ToolInputSchema {
                pub r#type: String,
                pub properties: Option<HashMap<String, JSONValue>>,
                pub required: Option<Vec<String>>,
            }
        }
    }

    pub mod error {
        pub struct MCPError;
    }

    pub mod macros {
        #[cfg(feature = "macros")]
        pub use ::mcpr_macros::*;
    }
}

// Implementation with the macro
#[cfg(feature = "macros")]
#[mcp_server]
impl McpServer for MockServer {
    // We need to provide the tools_list method since this is a test
    fn tools_list(&self) -> Result<Vec<Tool>> {
        Ok(Vec::new())
    }
}

fn main() {
    // This is just here to make trybuild happy
}
