//! Templates for generating MCP server and client stubs with stdio transport

/// Template for project server main.rs with stdio transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"use log::{debug, error, info};
use mcpr::schema::json_rpc::{JSONRPCErrorObject, JSONRPCMessage, JSONRPCResponse, RequestId};
use mcpr::schema::common::{Tool, ToolInputSchema};
use mcpr::server::{Server, ServerConfig};
use mcpr::transport::stdio::StdioTransport;
use mcpr::error::MCPError;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Configure the server
    let server_config = ServerConfig::new()
        .with_name("{{name}}-server")
        .with_version("1.0.0")
        .with_tool(Tool {
            name: "hello".to_string(),
            description: Some("A simple hello world tool".to_string()),
            input_schema: ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some([
                    ("name".to_string(), serde_json::json!({
                        "type": "string",
                        "description": "Name to greet"
                    }))
                ].into_iter().collect()),
                required: Some(vec!["name".to_string()]),
            },
        });
    
    // Create a transport
    let transport = StdioTransport::new();
    
    // Create a server
    let mut server = Server::new(server_config);
    
    // Register tools
    server.register_tool_handler("hello", |params: Value| async move {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("World");
            
        Ok(serde_json::json!({
            "message": format!("Hello, {}!", name)
        }))
    })?;
    
    // Start the server
    info!("Starting MCP server with stdio transport");
    server.serve(transport).await?;

    Ok(())
}"#;

/// Template for project client main.rs with stdio transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with stdio transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema},
    transport::{
        stdio::StdioTransport,
        Transport,
    },
};
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;
use log::{info, error, debug, warn};
use mcpr::schema::json_rpc::{JSONRPCErrorObject, JSONRPCMessage, JSONRPCResponse, RequestId};
use mcpr::client::Client;

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
}

/// Tool handler function type
type ToolHandler = Box<dyn Fn(Value) -> Result<Value, MCPError> + Send + Sync>;

/// High-level MCP client
struct StdioClient<T> {
    transport: T,
}

impl<T> StdioClient<T> 
where 
    T: Transport
{
    /// Create a new MCP client with the given transport
    fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Connect to the server
    async fn connect(&mut self) -> Result<(), MCPError> {
        info!("Connecting to server...");
        
        loop {
            let message = {
                let transport = &mut self.transport;

                // Receive a message
                match transport.receive().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        // For transport errors, log them but continue waiting
                        // This allows the client to keep trying to connect even if there are temporary connection issues
                        error!("Transport error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        continue;
                    }
                }
            };

            // Handle the message
            match message {
                mcpr::schema::json_rpc::JSONRPCMessage::Request(request) => {
                    let id = request.id.clone();
                    let method = request.method.clone();
                    let params = request.params.clone();

                    match method.as_str() {
                        "initialize" => {
                            info!("Received initialization request");
                            self.handle_initialize(id, params).await?;
                        }
                        "tool_call" => {
                            info!("Received tool call request");
                            self.handle_tool_call(id, params).await?;
                        }
                        "shutdown" => {
                            info!("Received shutdown request");
                            self.handle_shutdown(id).await?;
                            break;
                        }
                        _ => {
                            warn!("Unknown method: {}", method);
                            self.send_error(
                                id,
                                -32601,
                                format!("Method not found: {}", method),
                                None,
                            ).await?;
                        }
                    }
                }
                _ => {
                    warn!("Unexpected message type");
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Handle initialization request
    async fn handle_initialize(&mut self, id: mcpr::schema::json_rpc::RequestId, _params: Option<Value>) -> Result<(), MCPError> {
        let transport = &mut self.transport;

        // Create initialization response
        let response = mcpr::schema::json_rpc::JSONRPCResponse::new(
            id,
            serde_json::json!({
                "protocol_version": mcpr::constants::LATEST_PROTOCOL_VERSION,
                "server_info": {
                    "name": "{{name}}-server",
                    "version": "1.0.0"
                },
                "tools": [
                    {
                        "name": "hello",
                        "description": "A simple hello world tool",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Name to greet"
                                }
                            },
                            "required": ["name"]
                        }
                    }
                ]
            }),
        );

        // Send the response
        debug!("Sending initialization response");
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle tool call request
    async fn handle_tool_call(&mut self, id: mcpr::schema::json_rpc::RequestId, params: Option<Value>) -> Result<(), MCPError> {
        let transport = &mut self.transport;

        // Extract tool name and parameters
        let params = params.ok_or_else(|| {
            MCPError::Protocol("Missing parameters in tool call request".to_string())
        })?;

        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::Protocol("Missing tool name in parameters".to_string()))?;

        let tool_params = params.get("parameters").cloned().unwrap_or(Value::Null);
        debug!("Tool call: {} with parameters: {:?}", tool_name, tool_params);

        // Find the tool handler
        let handler = match tool_name {
            "hello" => Box::new(|params: Value| {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("World");
                
                Ok(serde_json::json!({
                    "message": format!("Hello, {}!", name)
                }))
            }) as ToolHandler,
            _ => return Err(MCPError::Protocol(format!("No handler registered for tool '{}'", tool_name))),
        };

        // Call the handler
        match handler(tool_params) {
            Ok(result) => {
                // Create tool result response
                let response = mcpr::schema::json_rpc::JSONRPCResponse::new(id, result);
                
                // Send the response
                debug!("Sending tool call response: {:?}", response);
                transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response)).await?;
            }
            Err(e) => {
                // Create error response
                let error_obj = mcpr::schema::json_rpc::JSONRPCErrorObject {
                    code: -32000,
                    message: format!("Tool call failed: {}", e),
                    data: None
                };
                let error = mcpr::schema::json_rpc::JSONRPCError::new(id, error_obj);
                
                // Send the error response
                debug!("Sending tool call error response: {:?}", error);
                transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Error(error)).await?;
            }
        }
        
        Ok(())
    }

    /// Handle shutdown request
    async fn handle_shutdown(&mut self, id: mcpr::schema::json_rpc::RequestId) -> Result<(), MCPError> {
        let transport = &mut self.transport;

        // Create shutdown response
        let response = mcpr::schema::json_rpc::JSONRPCResponse::new(id, serde_json::json!({}));

        // Send the response
        debug!("Sending shutdown response");
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response)).await?;

        // Close the transport
        info!("Closing transport");
        transport.close().await?;

        Ok(())
    }

    /// Send an error response
    async fn send_error(
        &mut self,
        id: mcpr::schema::json_rpc::RequestId,
        code: i32,
        message: String,
        data: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = &mut self.transport;

        // Create error response
        let error_obj = mcpr::schema::json_rpc::JSONRPCErrorObject {
            code,
            message: message.clone(),
            data
        };
        let error = mcpr::schema::json_rpc::JSONRPCMessage::Error(
            mcpr::schema::json_rpc::JSONRPCError::new(id, error_obj),
        );

        // Send the error
        warn!("Sending error response: {}", message);
        transport.send(&error).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    if args.debug {
        std::env::set_var("RUST_LOG", "debug,mcpr=debug");
    } else {
        std::env::set_var("RUST_LOG", "info,mcpr=info");
    }
    env_logger::init();

    info!("Starting MCP client for {{name}} project with stdio transport");
    
    // Create a transport
    let transport = StdioTransport::new();
    
    // Create a client and connect to the server
    let mut client = StdioClient::new(transport);
    client.connect().await?;
    
    info!("Client shutdown complete");
    
    Ok(())
}"#;

/// Template for project server Cargo.toml with stdio transport
pub const PROJECT_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
mcpr = { path = "../mcpr" }
clap = { version = "4.0", features = ["derive"] }
serde = "1.0"
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
anyhow = "1.0"
thiserror = "1.0"
tokio = { version = "1", features = ["full"] }
"#;

/// Template for project client Cargo.toml with stdio transport
pub const PROJECT_CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-client"
version = "0.1.0"
edition = "2021"
description = "MCP client for {{name}} project with stdio transport"

[dependencies]
# For local development, use path dependency:
# mcpr = { path = "../.." }
# For production, use version from crates.io:
mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
# Additional dependencies for improved client
anyhow = "1.0"
thiserror = "1.0"
tokio = { version = "1", features = ["full"] }
"#;

/// Template for project test script with stdio transport
pub const PROJECT_TEST_SCRIPT_TEMPLATE: &str = r#"#!/bin/bash

# Test script for {{name}} MCP project with stdio transport

# Exit on error
set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building server...${NC}"
cd server
cargo build

echo -e "${BLUE}Building client...${NC}"
cd ../client
cargo build

echo -e "${BLUE}Testing Method 1: Direct JSON-RPC communication${NC}"
cd ..
echo -e "${GREEN}Creating a test input file...${NC}"
cat > test_input.json << EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol_version":"2024-11-05"}}
{"jsonrpc":"2.0","id":2,"method":"tool_call","params":{"name":"hello","parameters":{"name":"MCP User"}}}
{"jsonrpc":"2.0","id":3,"method":"shutdown","params":{}}
EOF

echo -e "${GREEN}Running server with test input...${NC}"
./server/target/debug/{{name}}-server < test_input.json > test_output.json

echo -e "${GREEN}Checking server output...${NC}"
if grep -q "Hello, MCP User" test_output.json; then
    echo -e "${GREEN}Direct JSON-RPC test completed successfully!${NC}"
else
    echo -e "${RED}Direct JSON-RPC test failed. Server output does not contain expected response.${NC}"
    cat test_output.json
    exit 1
fi

# Clean up
rm test_input.json test_output.json

echo -e "${BLUE}Testing Method 2: Client starting server${NC}"
echo -e "${GREEN}Running client in one-shot mode...${NC}"
./client/target/debug/{{name}}-client --name "MCP Tester" > client_output.txt

echo -e "${GREEN}Checking client output...${NC}"
if grep -q "Hello, MCP Tester" client_output.txt; then
    echo -e "${GREEN}Client-server test completed successfully!${NC}"
else
    echo -e "${RED}Client-server test failed. Client output does not contain expected response.${NC}"
    cat client_output.txt
    exit 1
fi

# Clean up
rm client_output.txt

echo -e "${BLUE}Testing Method 3: Client connecting to running server${NC}"
echo -e "${GREEN}Starting server in background...${NC}"
./server/target/debug/{{name}}-server &
SERVER_PID=$!

# Give the server a moment to start
sleep 1

echo -e "${GREEN}Running client in connect mode...${NC}"
./client/target/debug/{{name}}-client --connect --name "Connected User" > connect_output.txt

echo -e "${GREEN}Checking client output...${NC}"
if grep -q "Hello, Connected User" connect_output.txt; then
    echo -e "${GREEN}Connect mode test completed successfully!${NC}"
else
    echo -e "${RED}Connect mode test failed. Client output does not contain expected response.${NC}"
    cat connect_output.txt
    kill $SERVER_PID
    exit 1
fi

# Clean up
rm connect_output.txt
kill $SERVER_PID

echo -e "${GREEN}All tests completed successfully!${NC}"
"#;
