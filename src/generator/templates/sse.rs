//! Templates for generating MCP server and client stubs with SSE transport

/// Template for project server main.rs with SSE transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"//! MCP Server for {{name}} project with SSE transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema},
    transport::{
        sse::SSEServerTransport,
        Transport,
    },
};
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

/// Server configuration
struct ServerConfig {
    /// Server name
    name: String,
    /// Server version
    version: String,
    /// Available tools
    tools: Vec<Tool>,
}

impl ServerConfig {
    /// Create a new server configuration
    fn new() -> Self {
        Self {
            name: "MCP Server".to_string(),
            version: "1.0.0".to_string(),
            tools: Vec::new(),
        }
    }

    /// Set the server name
    fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set the server version
    fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    /// Add a tool to the server
    fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }
}

/// Tool handler function type
type ToolHandler = Box<dyn Fn(Value) -> Result<Value, MCPError> + Send + Sync>;

/// High-level MCP server
struct Server<T> {
    config: ServerConfig,
    tool_handlers: HashMap<String, ToolHandler>,
    transport: Option<T>,
}

impl<T> Server<T> 
where 
    T: Transport
{
    /// Create a new MCP server with the given configuration
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            tool_handlers: HashMap::new(),
            transport: None,
        }
    }

    /// Register a tool handler
    fn register_tool_handler<F>(&mut self, tool_name: &str, handler: F) -> Result<(), MCPError>
    where
        F: Fn(Value) -> Result<Value, MCPError> + Send + Sync + 'static,
    {
        // Check if the tool exists in the configuration
        if !self.config.tools.iter().any(|t| t.name == tool_name) {
            return Err(MCPError::Protocol(format!(
                "Tool '{}' not found in server configuration",
                tool_name
            )));
        }

        // Register the handler
        self.tool_handlers
            .insert(tool_name.to_string(), Box::new(handler));

        info!("Registered handler for tool '{}'", tool_name);
        Ok(())
    }

    /// Start the server with the given transport
    async fn start(&mut self, mut transport: T) -> Result<(), MCPError> {
        // Start the transport
        info!("Starting transport...");
        transport.start().await?;

        // Store the transport
        self.transport = Some(transport);

        // Process messages
        info!("Processing messages...");
        self.process_messages().await
    }

    /// Process incoming messages
    async fn process_messages(&mut self) -> Result<(), MCPError> {
        info!("Server is running and waiting for client connections...");
        
        loop {
            let message = {
                let transport = self
                    .transport
                    .as_mut()
                    .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

                // Receive a message
                match transport.receive().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        // For transport errors, log them but continue waiting
                        // This allows the server to keep running even if there are temporary connection issues
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
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create initialization response
        let response = mcpr::schema::json_rpc::JSONRPCResponse::new(
            id,
            serde_json::json!({
                "protocol_version": mcpr::constants::LATEST_PROTOCOL_VERSION,
                "server_info": {
                    "name": self.config.name,
                    "version": self.config.version
                },
                "tools": self.config.tools
            }),
        );

        // Send the response
        debug!("Sending initialization response");
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle tool call request
    async fn handle_tool_call(&mut self, id: mcpr::schema::json_rpc::RequestId, params: Option<Value>) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

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
        let handler = self.tool_handlers.get(tool_name).ok_or_else(|| {
            MCPError::Protocol(format!("No handler registered for tool '{}'", tool_name))
        })?;

        // Call the handler
        match handler(tool_params) {
            Ok(result) => {
                // Create tool call response
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
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

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
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

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

/// Start the server
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

    // Create the server configuration
    let config = ServerConfig::new();
    
    // Create the tools
    let hello_tool = Tool {
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
    };
    
    // Create the server
    let mut server = mcpr::server::Server::new(
        mcpr::server::ServerConfig::new()
            .with_name("{{name}}-server")
            .with_version("1.0.0")
            .with_tool(hello_tool)
    );
    
    // Register tool handlers
    server.register_tool_handler("hello", |params: Value| async move {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("World");
            
        let result = serde_json::json!({
            "message": format!("Hello, {}!", name)
        });
        
        Ok(result)
    })?;
    
    // Create a transport
    let uri = format!("http://0.0.0.0:{}", args.port);
    let transport = SSEServerTransport::new(&uri)?;
    
    // Start the server
    info!("Starting MCP server with SSE transport on {}", uri);
    info!("Endpoints:");
    info!("  - GET  {}/events   (SSE events stream)", uri);
    info!("  - POST {}/messages (Message endpoint)", uri);
    
    server.serve(transport).await?;
    
    Ok(())
}"#;

/// Template for project client main.rs with SSE transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with SSE transport

use clap::Parser;
use mcpr::{
    client::Client,
    error::MCPError,
    transport::{
        sse::SSEClientTransport,
        Transport,
    },
};
use serde_json::Value;
use std::error::Error;
use std::io::{self, Write};
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// URI of the server
    #[arg(short, long, default_value = "http://localhost:8080")]
    uri: String,
    
    /// Enable interactive mode
    #[arg(short, long)]
    interactive: bool,
    
    /// Name to use for hello tool
    #[arg(short, long, default_value = "World")]
    name: String,
}

/// Prompt for user input
fn prompt_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    Ok(input.trim().to_string())
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

    // Create a transport
    let server_url = args.uri.clone();
    info!("Connecting to server: {}", server_url);
    let transport = SSEClientTransport::new(&server_url)?;
    
    // Create a client
    let mut client = Client::new(transport);

    // Initialize client
    info!("Initializing client...");
    let init_result = client.initialize().await?;
    
    // Get server information
    if let Some(server_info) = init_result.get("server_info") {
        info!("Server info: {}", serde_json::to_string(&server_info)?);
        
        println!("Available tools:");
        if let Some(tools) = init_result.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                println!("  - {}: {}", 
                    tool.get("name").and_then(|n| n.as_str()).unwrap_or("unknown"),
                    tool.get("description").and_then(|d| d.as_str()).unwrap_or("No description"));
            }
        } else {
            println!("  No tools available");
        }
    }
    
    // Handle interactive or one-shot mode
    if args.interactive {
        info!("Running in interactive mode");
        
        // Interactive loop
        loop {
            let tool_name = prompt_input("Enter tool name (or 'exit' to quit)")?;
            
            if tool_name.to_lowercase() == "exit" {
                break;
            }
            
            // Check if the tool exists in the available tools
            let tool_exists = init_result.get("tools")
                .and_then(|t| t.as_array())
                .map(|tools| tools.iter().any(|t| t.get("name").and_then(|n| n.as_str()) == Some(&tool_name)))
                .unwrap_or(false);
                
            if tool_name.to_lowercase() == "hello" || tool_exists {
                let name = prompt_input("Enter name to greet")?;
                
                info!("Calling tool '{}' with parameters: {}", tool_name, name);
                match client.call_tool::<_, Value>(&tool_name, &serde_json::json!({
                    "name": name
                })).await {
                    Ok(response) => {
                        let message = response.get("message").and_then(|m| m.as_str()).unwrap_or("No message");
                        println!("{}", message);
                    },
                    Err(e) => {
                        error!("Tool call failed: {}", e);
                        println!("Error: {}", e);
                    }
                }
            } else {
                println!("Unknown tool: {}", tool_name);
            }
        }
    } else {
        // One-shot mode
        info!("Running in one-shot mode with name: {}", args.name);
        
        // Call the hello tool
        info!("Calling tool 'hello' with parameters: {}", serde_json::json!({"name": args.name}));
        let response: Value = client.call_tool("hello", &serde_json::json!({
            "name": args.name
        })).await?;
        
        info!("Received message: {}", response.get("message").and_then(|m| m.as_str()).unwrap_or("No message"));
        if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
            println!("{}", message);
        } else {
            println!("No message received");
        }
    }
    
    // Shutdown the client
    info!("Shutting down client");
    client.shutdown().await?;
    info!("Client shutdown complete");
    
    Ok(())
}"#;

/// Template for project server Cargo.toml with SSE transport
pub const PROJECT_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"
description = "MCP server for {{name}} project with SSE transport"

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
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
"#;

/// Template for project client Cargo.toml with SSE transport
pub const PROJECT_CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-client"
version = "0.1.0"
edition = "2021"
description = "MCP client for {{name}} project with SSE transport"

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
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
"#;

/// Template for project test script with SSE transport
pub const PROJECT_TEST_SCRIPT_TEMPLATE: &str = r#"#!/bin/bash

# Test script for {{name}} MCP project with SSE transport

# Exit on error
set -e

# Enable verbose output
set -x

# Function to clean up on exit
cleanup() {
  echo "Cleaning up..."
  if [ ! -z "$SERVER_PID" ]; then
    echo "Shutting down server (PID: $SERVER_PID)..."
    kill $SERVER_PID 2>/dev/null || true
  fi
  if [ ! -z "$CAT_PID" ]; then
    kill $CAT_PID 2>/dev/null || true
  fi
  if [ ! -z "$SERVER_PIPE" ] && [ -e "$SERVER_PIPE" ]; then
    rm $SERVER_PIPE 2>/dev/null || true
  fi
  exit $1
}

# Set up trap for clean exit
trap 'cleanup 1' INT TERM

echo "Building server..."
cd server
cargo build

echo "Building client..."
cd ../client
cargo build

# Create a named pipe for server output
SERVER_PIPE="/tmp/server_pipe_$$"
mkfifo $SERVER_PIPE

# Start reading from the pipe in the background
cat $SERVER_PIPE &
CAT_PID=$!

echo "Starting server in background..."
cd ..
RUST_LOG=debug,mcpr=trace ./server/target/debug/{{name}}-server --port 8081 > $SERVER_PIPE 2>&1 &
SERVER_PID=$!

# Give the server time to start
echo "Waiting for server to start..."
sleep 3

# Check if server is still running
if ! kill -0 $SERVER_PID 2>/dev/null; then
  echo "Error: Server failed to start or crashed"
  cleanup 1
fi

echo "Running client..."
RUST_LOG=debug,mcpr=trace ./client/target/debug/{{name}}-client --uri "http://localhost:8081" --name "MCP User"
CLIENT_EXIT=$?

if [ $CLIENT_EXIT -ne 0 ]; then
  echo "Error: Client exited with code $CLIENT_EXIT"
  cleanup $CLIENT_EXIT
fi

echo "Shutting down server..."
kill $SERVER_PID 2>/dev/null || true
kill $CAT_PID 2>/dev/null || true
rm $SERVER_PIPE 2>/dev/null || true
wait $SERVER_PID 2>/dev/null || true

echo "Test completed successfully!"
cleanup 0
"#;
