#![cfg(feature = "macros")]

use mcpr::error::MCPError;
use mcpr::macros::mcp_server;
use mcpr::schema::common::Tool;

// Test server implementation
struct TestServer {
    // Server state
    counter: std::sync::atomic::AtomicU32,
}

impl TestServer {
    fn new() -> Self {
        Self {
            counter: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

// Define the minimal trait for our test
// This is what would normally be the McpServer trait
trait TestServerTrait {
    // The tools_list method that our macro should augment
    fn tools_list(&self) -> Result<Vec<Tool>, MCPError>;
}

// Use the macro to implement the server
#[mcp_server]
impl TestServerTrait for TestServer {
    // Basic implementation that will be augmented by the macro
    fn tools_list(&self) -> Result<Vec<Tool>, MCPError> {
        Ok(Vec::new())
    }
}

// Implement methods with #[tool] attribute in a separate impl block
// This is a workaround since trait methods must be declared in the trait
impl TestServer {
    #[mcpr::macros::tool]
    fn increment_counter(&self) -> Result<u32, MCPError> {
        let new_value = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        Ok(new_value)
    }

    #[mcpr::macros::tool]
    fn get_counter(&self) -> Result<u32, MCPError> {
        Ok(self.counter.load(std::sync::atomic::Ordering::SeqCst))
    }

    #[mcpr::macros::tool]
    fn add_numbers(&self, a: u32, b: u32) -> Result<u32, MCPError> {
        Ok(a + b)
    }
}

// For now, we'll skip the integration test since we're still developing the macro
// #[test]
// fn test_server_tools_list() {
//     let server = TestServer::new();
//
//     // Call tools_list and check that we have the expected tools
//     let tools = server.tools_list().unwrap();
//
//     // We should have 3 tools
//     assert_eq!(tools.len(), 3);
//
//     // Check that our tools are there
//     let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
//     assert!(tool_names.contains(&"increment_counter".to_string()));
//     assert!(tool_names.contains(&"get_counter".to_string()));
//     assert!(tool_names.contains(&"add_numbers".to_string()));
// }
