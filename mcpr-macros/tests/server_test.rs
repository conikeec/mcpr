#![cfg_attr(not(test), doc = "")]
#![allow(dead_code, unused_variables)]

// This is a mock test file for the trybuild crate
// It's not meant to be compiled directly, only used for macro expansion testing

// Import the necessary macros for testing
use mcpr_macros::{mcp_server, tool};

// Create a namespace that matches the real crate
pub(crate) mod mcpr {
    pub(crate) mod schema {
        pub(crate) mod common {
            pub struct Tool {
                pub name: String,
                pub description: Option<String>,
                pub input_schema: ToolInputSchema,
            }

            pub struct ToolInputSchema {
                pub r#type: String,
                pub properties: Option<std::collections::HashMap<String, crate::JSONValue>>,
                pub required: Option<Vec<String>>,
            }
        }
    }

    pub(crate) mod error {
        #[derive(Debug)]
        pub struct MCPError;
    }
}

// Define a simple type for JSON values
pub struct JSONValue;

// Define simple types for the test
type Result<T> = std::result::Result<T, mcpr::error::MCPError>;

// Define a server trait
trait McpServer {
    fn tools_list(&self) -> Result<Vec<mcpr::schema::common::Tool>>;
    fn example_tool(&self, param1: String) -> Result<String>;
}

// Define a simple server
struct MockServer;

// Implement the server with the macro
#[mcp_server]
impl McpServer for MockServer {
    fn tools_list(&self) -> Result<Vec<mcpr::schema::common::Tool>> {
        Ok(Vec::new())
    }

    #[tool]
    fn example_tool(&self, param1: String) -> Result<String> {
        Ok(format!("Result for {}", param1))
    }
}

fn main() {
    // This is just here to make trybuild happy
    let server = MockServer;
    let _ = server.tools_list();
}
