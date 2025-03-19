#![allow(dead_code)]

// This is a mock test file for the trybuild crate
// It's not meant to be compiled directly, only used for macro expansion testing

// Import the necessary macros for testing
use mcpr_macros::mcp_client;

// Create a namespace that matches the real crate
pub(crate) mod mcpr {
    pub(crate) mod schema {
        pub(crate) mod common {
            // Common schema types
        }

        pub(crate) mod json_rpc {
            // Mock Value type for JSON
            pub struct Value;

            // Function for testing
            pub fn json(_args: &str) -> Value {
                Value
            }

            // JSON-RPC schema types
            pub enum RequestId {
                Number(i64),
                String(String),
            }

            pub struct JSONRPCRequest {
                pub id: RequestId,
                pub method: String,
                pub params: Option<Value>,
            }

            impl JSONRPCRequest {
                pub fn new(id: RequestId, method: String, params: Option<Value>) -> Self {
                    Self { id, method, params }
                }
            }

            pub enum JSONRPCMessage {
                Request(JSONRPCRequest),
                Response(JSONRPCResponse),
                Error(JSONRPCError),
            }

            pub struct JSONRPCResponse {
                pub id: RequestId,
                pub result: Value,
            }

            pub struct JSONRPCError {
                pub code: i32,
                pub message: String,
            }
        }
    }

    pub(crate) mod transport {
        // Transport trait
        pub trait Transport {
            fn start(&mut self) -> Result<(), super::error::MCPError>;
            fn close(&mut self) -> Result<(), super::error::MCPError>;
            fn send<T>(&mut self, message: &T) -> Result<(), super::error::MCPError>;
            fn receive<T>(&mut self) -> Result<T, super::error::MCPError>;
        }
    }

    pub(crate) mod error {
        // Error type
        #[derive(Debug)]
        pub enum MCPError {
            Protocol(String),
            Transport(String),
            Serialization(String),
        }
    }

    pub(crate) mod constants {
        pub const LATEST_PROTOCOL_VERSION: &str = "0.1.0";
    }
}

// Define simple types for the test
type Result<T, E = mcpr::error::MCPError> = std::result::Result<T, E>;

// Define a client trait
trait McpClient {
    fn search_docs(&self, query: String) -> Result<String>;
    fn get_file_content(&self, path: String, limit: u32) -> Result<String>;
}

// Define a simple client
struct MockClient;

// Implement the client with the macro
#[mcp_client]
impl McpClient for MockClient {
    fn search_docs(&self, _query: String) -> Result<String> {
        Ok("Search results".to_string())
    }

    fn get_file_content(&self, _path: String, _limit: u32) -> Result<String> {
        Ok("File content".to_string())
    }
}

fn main() {
    // This is just here to make trybuild happy
    let _client = MockClient;
    // In real code, we would call the methods
}
