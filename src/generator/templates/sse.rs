//! Templates for generating MCP server and client stubs with SSE transport

/// Template for project server main.rs with SSE transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"//! MCP Server for {{name}} project with SSE transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::{
        common::{Tool, ToolInputSchema},
        json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    },
    transport::{
        sse::SSETransport,
        Transport,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
    fn start(&mut self, mut transport: T) -> Result<(), MCPError> {
        // Start the transport
        info!("Starting transport...");
        transport.start()?;

        // Store the transport
        self.transport = Some(transport);

        // Process messages
        info!("Processing messages...");
        self.process_messages()
    }

    /// Process incoming messages
    fn process_messages(&mut self) -> Result<(), MCPError> {
        info!("Server is running and waiting for client connections...");
        
        loop {
            let message = {
                let transport = self
                    .transport
                    .as_mut()
                    .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

                // Receive a message
                match transport.receive() {
                    Ok(msg) => msg,
                    Err(e) => {
                        // For transport errors, log them but continue waiting
                        // This allows the server to keep running even if there are temporary connection issues
                        error!("Transport error: {}", e);
                        std::thread::sleep(std::time::Duration::from_millis(1000));
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
                            self.handle_initialize(id, params)?;
                        }
                        "tool_call" => {
                            info!("Received tool call request");
                            self.handle_tool_call(id, params)?;
                        }
                        "shutdown" => {
                            info!("Received shutdown request");
                            self.handle_shutdown(id)?;
                            break;
                        }
                        _ => {
                            warn!("Unknown method: {}", method);
                            self.send_error(
                                id,
                                -32601,
                                format!("Method not found: {}", method),
                                None,
                            )?;
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
    fn handle_initialize(&mut self, id: mcpr::schema::json_rpc::RequestId, _params: Option<Value>) -> Result<(), MCPError> {
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
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response))?;

        Ok(())
    }

    /// Handle tool call request
    fn handle_tool_call(&mut self, id: mcpr::schema::json_rpc::RequestId, params: Option<Value>) -> Result<(), MCPError> {
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
                transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response))?;
            }
            Err(e) => {
                // Create error response
                let error = mcpr::schema::json_rpc::JSONRPCError::new(
                    id,
                    -32000,
                    format!("Tool call failed: {}", e),
                    None,
                );

                // Send the error response
                debug!("Sending tool call error response: {:?}", error);
                transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Error(error))?;
            }
        }

        Ok(())
    }

    /// Handle shutdown request
    fn handle_shutdown(&mut self, id: mcpr::schema::json_rpc::RequestId) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create shutdown response
        let response = mcpr::schema::json_rpc::JSONRPCResponse::new(id, serde_json::json!({}));

        // Send the response
        debug!("Sending shutdown response");
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response))?;

        // Close the transport
        info!("Closing transport");
        transport.close()?;

        Ok(())
    }

    /// Send an error response
    fn send_error(
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
        let error = mcpr::schema::json_rpc::JSONRPCMessage::Error(
            mcpr::schema::json_rpc::JSONRPCError::new(id, code, message.clone(), data),
        );

        // Send the error
        warn!("Sending error response: {}", message);
        transport.send(&error)?;

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Set log level based on debug flag
    if args.debug {
        log::set_max_level(log::LevelFilter::Debug);
        debug!("Debug logging enabled");
    }
    
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
    
    // Create the server
    let mut server = Server::new(server_config);
    
    // Register tool handlers
    server.register_tool_handler("hello", |params: Value| {
        // Parse parameters
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::Protocol("Missing name parameter".to_string()))?;
        
        info!("Handling hello tool call for name: {}", name);
        
        // Generate response
        let response = serde_json::json!({
            "message": format!("Hello, {}!", name)
        });
        
        Ok(response)
    })?;
    
    // Create transport and start the server
    let uri = format!("http://localhost:{}", args.port);
    info!("Starting SSE server on {}", uri);
    let transport = SSETransport::new_server(&uri);
    
    info!("Starting {{name}}-server...");
    server.start(transport)?;
    
    Ok(())
}"#;

/// Template for project client main.rs with SSE transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with SSE transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    transport::{
        sse::SSETransport,
        Transport,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Server URI
    #[arg(short, long, default_value = "http://localhost:8080")]
    uri: String,
    
    /// Run in interactive mode
    #[arg(short, long)]
    interactive: bool,
    
    /// Name to greet (for non-interactive mode)
    #[arg(short, long)]
    name: Option<String>,
}

/// High-level MCP client
struct Client<T: Transport> {
    transport: T,
    next_request_id: i64,
}

impl<T: Transport> Client<T> {
    /// Create a new MCP client with the given transport
    fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: 1,
        }
    }

    /// Initialize the client
    fn initialize(&mut self) -> Result<Value, MCPError> {
        // Start the transport
        debug!("Starting transport");
        self.transport.start()?;

        // Send initialization request
        let initialize_request = JSONRPCRequest::new(
            self.next_request_id(),
            "initialize".to_string(),
            Some(serde_json::json!({
                "protocol_version": mcpr::constants::LATEST_PROTOCOL_VERSION
            })),
        );

        let message = JSONRPCMessage::Request(initialize_request);
        debug!("Sending initialize request: {:?}", message);
        self.transport.send(&message)?;

        // Wait for response
        info!("Waiting for initialization response");
        let response: JSONRPCMessage = self.transport.receive()?;
        debug!("Received response: {:?}", response);

        match response {
            JSONRPCMessage::Response(resp) => Ok(resp.result),
            JSONRPCMessage::Error(err) => {
                error!("Initialization failed: {:?}", err);
                Err(MCPError::Protocol(format!(
                    "Initialization failed: {:?}",
                    err
                )))
            }
            _ => {
                error!("Unexpected response type");
                Err(MCPError::Protocol("Unexpected response type".to_string()))
            }
        }
    }

    /// Call a tool on the server
    fn call_tool<P: Serialize + std::fmt::Debug, R: DeserializeOwned>(
        &mut self,
        tool_name: &str,
        params: &P,
    ) -> Result<R, MCPError> {
        // Create tool call request
        let tool_call_request = JSONRPCRequest::new(
            self.next_request_id(),
            "tool_call".to_string(),
            Some(serde_json::json!({
                "name": tool_name,
                "parameters": serde_json::to_value(params)?
            })),
        );

        let message = JSONRPCMessage::Request(tool_call_request);
        info!("Calling tool '{}' with parameters: {:?}", tool_name, params);
        debug!("Sending tool call request: {:?}", message);
        self.transport.send(&message)?;

        // Wait for response
        info!("Waiting for tool call response");
        let response: JSONRPCMessage = self.transport.receive()?;
        debug!("Received response: {:?}", response);

        match response {
            JSONRPCMessage::Response(resp) => {
                // Extract the tool result from the response
                let result_value = resp.result;
                
                // Parse the result
                debug!("Parsing result: {:?}", result_value);
                serde_json::from_value(result_value).map_err(|e| {
                    error!("Failed to parse result: {}", e);
                    MCPError::Serialization(e)
                })
            }
            JSONRPCMessage::Error(err) => {
                error!("Tool call failed: {:?}", err);
                Err(MCPError::Protocol(format!("Tool call failed: {:?}", err)))
            }
            _ => {
                error!("Unexpected response type");
                Err(MCPError::Protocol("Unexpected response type".to_string()))
            }
        }
    }

    /// Shutdown the client
    fn shutdown(&mut self) -> Result<(), MCPError> {
        // Send shutdown request
        let shutdown_request =
            JSONRPCRequest::new(self.next_request_id(), "shutdown".to_string(), None);

        let message = JSONRPCMessage::Request(shutdown_request);
        info!("Sending shutdown request");
        debug!("Shutdown request: {:?}", message);
        self.transport.send(&message)?;

        // Wait for response
        info!("Waiting for shutdown response");
        let response: JSONRPCMessage = self.transport.receive()?;
        debug!("Received response: {:?}", response);

        match response {
            JSONRPCMessage::Response(_) => {
                // Close the transport
                info!("Closing transport");
                self.transport.close()?;
                Ok(())
            }
            JSONRPCMessage::Error(err) => {
                error!("Shutdown failed: {:?}", err);
                Err(MCPError::Protocol(format!("Shutdown failed: {:?}", err)))
            }
            _ => {
                error!("Unexpected response type");
                Err(MCPError::Protocol("Unexpected response type".to_string()))
            }
        }
    }

    /// Generate the next request ID
    fn next_request_id(&mut self) -> RequestId {
        let id = self.next_request_id;
        self.next_request_id += 1;
        RequestId::Number(id)
    }
}

fn prompt_input(prompt: &str) -> Result<String, io::Error> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    Ok(input.trim().to_string())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Set log level based on debug flag
    if args.debug {
        log::set_max_level(log::LevelFilter::Debug);
        debug!("Debug logging enabled");
    }
    
    // Create transport and client
    info!("Using SSE transport with URI: {}", args.uri);
    let transport = SSETransport::new(&args.uri);
    
    let mut client = Client::new(transport);
    
    // Initialize the client
    info!("Initializing client...");
    let init_result = match client.initialize() {
        Ok(result) => {
            info!("Server info: {:?}", result);
            result
        },
        Err(e) => {
            error!("Failed to initialize client: {}", e);
            return Err(Box::new(e));
        }
    };
    
    if args.interactive {
        // Interactive mode
        info!("=== {{name}}-client Interactive Mode ===");
        println!("=== {{name}}-client Interactive Mode ===");
        println!("Type 'exit' or 'quit' to exit");
        
        loop {
            let name = prompt_input("Enter your name (or 'exit' to quit)")?;
            if name.to_lowercase() == "exit" || name.to_lowercase() == "quit" {
                info!("User requested exit");
                break;
            }
            
            // Call the hello tool
            let request = serde_json::json!({
                "name": name
            });
            
            match client.call_tool::<Value, Value>("hello", &request) {
                Ok(response) => {
                    if let Some(message) = response.get("message") {
                        let msg = message.as_str().unwrap_or("");
                        info!("Received message: {}", msg);
                        println!("{}", msg);
                    } else {
                        info!("Received response without message field: {:?}", response);
                        println!("Response: {:?}", response);
                    }
                },
                Err(e) => {
                    error!("Error calling tool: {}", e);
                    eprintln!("Error: {}", e);
                }
            }
            
            println!();
        }
        
        info!("Exiting interactive mode");
        println!("Exiting interactive mode");
    } else {
        // One-shot mode
        let name = args.name.ok_or_else(|| {
            error!("Name is required in non-interactive mode");
            "Name is required in non-interactive mode"
        })?;
        
        info!("Running in one-shot mode with name: {}", name);
        
        // Call the hello tool
        let request = serde_json::json!({
            "name": name
        });
        
        let response: Value = match client.call_tool("hello", &request) {
            Ok(response) => response,
            Err(e) => {
                error!("Error calling tool: {}", e);
                return Err(Box::new(e));
            }
        };
        
        if let Some(message) = response.get("message") {
            let msg = message.as_str().unwrap_or("");
            info!("Received message: {}", msg);
            println!("{}", msg);
        } else {
            info!("Received response without message field: {:?}", response);
            println!("Response: {:?}", response);
        }
    }
    
    // Shutdown the client
    info!("Shutting down client");
    if let Err(e) = client.shutdown() {
        error!("Error during shutdown: {}", e);
    }
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
mcpr = "0.2"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
reqwest = { version = "0.11", features = ["blocking", "json"] }
"#;

/// Template for project client Cargo.toml with SSE transport
pub const PROJECT_CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-client"
version = "0.1.0"
edition = "2021"
description = "MCP client for {{name}} project with SSE transport"

[dependencies]
mcpr = "0.2"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
reqwest = { version = "0.11", features = ["blocking", "json"] }
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
