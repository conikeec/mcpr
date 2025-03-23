//! Templates for generating MCP server and client stubs with stdio transport

/// Template for project server main.rs with stdio transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"//! MCP Server for {{name}} project with stdio transport

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

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
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
                        "tools/list" => {
                            info!("Received tool list request");
                            self.handle_tool_list(id, params)?;
                        }
                        "tools/call" => {
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
                "protocolVersion": mcpr::constants::LATEST_PROTOCOL_VERSION,
                "serverInfo": {
                    "name": self.config.name,
                    "version": self.config.version
                },
                "capabilities": {
                    "tools": {}
                }
            }),
        );

        // Send the response
        debug!("Sending initialization response");
        transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response))?;

        Ok(())
    }

    /// Handle tool list
    fn handle_tool_list(&mut self, id: mcpr::schema::json_rpc::RequestId, _params: Option<Value>) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create initialization response
        let response = mcpr::schema::json_rpc::JSONRPCResponse::new(
            id,
            serde_json::json!({
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

        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
        debug!("Tool call: {} with arguments: {:?}", tool_name, arguments);

        // Find the tool handler
        let handler = self.tool_handlers.get(tool_name).ok_or_else(|| {
            MCPError::Protocol(format!("No handler registered for tool '{}'", tool_name))
        })?;

        // Call the handler
        match handler(arguments) {
            Ok(result) => {
                // Create tool result response
                let response = mcpr::schema::json_rpc::JSONRPCResponse::new(
                    id,
                    serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string(&result)?
                            }
                        ]
                    }),
                );

                // Send the response
                debug!("Sending tool call response: {:?}", result);
                transport.send(&mcpr::schema::json_rpc::JSONRPCMessage::Response(response))?;
            }
            Err(e) => {
                // Send error response
                error!("Tool execution failed: {}", e);
                self.send_error(id, -32000, format!("Tool execution failed: {}", e), None)?;
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
            mcpr::schema::json_rpc::JSONRPCError::new_with_details(id, code, message.clone(), data),
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
    server.register_tool_handler("hello", |arguments: Value| {
        // Parse arguments
        let name = arguments.get("name")
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
    info!("Starting stdio server");
    let transport = StdioTransport::new();
    
    info!("Starting {{name}}-server...");
    server.start(transport)?;
    
    Ok(())
}"#;

/// Template for project client main.rs with stdio transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with stdio transport
//!
//! This client demonstrates how to connect to an MCP server using stdio transport.
//! 
//! There are two ways to use this client:
//! 1. Connect to an already running server (recommended for production)
//! 2. Start a new server process and connect to it (convenient for development)
//!
//! The client supports both interactive and one-shot modes.

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    transport::{
        stdio::StdioTransport,
        Transport,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::error::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Server command to execute (if not connecting to an existing server)
    #[arg(short, long, default_value = "./server/target/debug/{{name}}-server")]
    server_cmd: String,
    
    /// Connect to an already running server instead of starting a new one
    #[arg(short, long)]
    connect: bool,
    
    /// Run in interactive mode
    #[arg(short, long)]
    interactive: bool,
    
    /// Name to greet (for non-interactive mode)
    #[arg(short, long)]
    name: Option<String>,
    
    /// Timeout in seconds for operations
    #[arg(short, long, default_value = "30")]
    timeout: u64,
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
                let result = result_value.get("result").ok_or_else(|| {
                    error!("Missing 'result' field in response");
                    MCPError::Protocol("Missing 'result' field in response".to_string())
                })?;

                // Parse the result
                debug!("Parsing result: {:?}", result);
                serde_json::from_value(result.clone()).map_err(|e| {
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

/// Connect to an already running server
fn connect_to_running_server(command: &str, args: &[&str]) -> Result<(StdioTransport, Option<Child>), Box<dyn Error>> {
    info!("Connecting to running server with command: {} {}", command, args.join(" "));
    
    // Start a new process that will connect to the server
    let mut process = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    // Create a stderr reader to monitor server output
    if let Some(stderr) = process.stderr.take() {
        let stderr_reader = BufReader::new(stderr);
        thread::spawn(move || {
            for line in stderr_reader.lines().map_while(Result::ok) {
                debug!("Server stderr: {}", line);
            }
        });
    }
    
    // Give the server a moment to start up
    thread::sleep(Duration::from_millis(500));
    
    // Create a transport that communicates with the server process
    let transport = StdioTransport::with_reader_writer(
        Box::new(process.stdout.take().ok_or("Failed to get stdout")?),
        Box::new(process.stdin.take().ok_or("Failed to get stdin")?),
    );
    
    Ok((transport, Some(process)))
}

/// Start a new server and connect to it
fn start_and_connect_to_server(server_cmd: &str) -> Result<(StdioTransport, Option<Child>), Box<dyn Error>> {
    info!("Starting server process: {}", server_cmd);
    
    // Start the server process
    let mut server_process = Command::new(server_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    // Create a stderr reader to monitor server output
    if let Some(stderr) = server_process.stderr.take() {
        let stderr_reader = BufReader::new(stderr);
        thread::spawn(move || {
            for line in stderr_reader.lines().map_while(Result::ok) {
                debug!("Server stderr: {}", line);
            }
        });
    }
    
    // Give the server a moment to start up
    thread::sleep(Duration::from_millis(500));
    
    let server_stdin = server_process.stdin.take().ok_or("Failed to get stdin")?;
    let server_stdout = server_process.stdout.take().ok_or("Failed to get stdout")?;

    info!("Using stdio transport");
    let transport = StdioTransport::with_reader_writer(
        Box::new(server_stdout),
        Box::new(server_stdin),
    );
    
    Ok((transport, Some(server_process)))
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
    
    // Set timeout
    let timeout = Duration::from_secs(args.timeout);
    info!("Operation timeout set to {} seconds", args.timeout);
    
    // Create transport and server process based on connection mode
    let (transport, server_process) = if args.connect {
        info!("Connecting to already running server");
        connect_to_running_server(&args.server_cmd, &[])?
    } else {
        info!("Starting new server process");
        start_and_connect_to_server(&args.server_cmd)?
    };
    
    let mut client = Client::new(transport);
    
    // Initialize the client with timeout
    info!("Initializing client...");
    let start_time = Instant::now();
    let _init_result = loop {
        if start_time.elapsed() >= timeout {
            error!("Initialization timed out after {:?}", timeout);
            return Err(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Initialization timed out after {:?}", timeout),
            )));
        }
        
        match client.initialize() {
            Ok(result) => {
                info!("Server info: {:?}", result);
                break result;
            },
            Err(e) => {
                warn!("Initialization attempt failed: {}", e);
                thread::sleep(Duration::from_millis(500));
                continue;
            }
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
    
    // If we started the server, terminate it gracefully
    if let Some(mut process) = server_process {
        info!("Terminating server process...");
        let _ = process.kill();
    }
    
    Ok(())
}"#;

/// Template for project server Cargo.toml with stdio transport
pub const PROJECT_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"
description = "MCP server for {{name}} project with stdio transport"

[dependencies]
# For local development, use path dependency:
mcpr = { path = "../../mcpr" }
# For production, use version from crates.io:
# mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
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

pub const PROJECT_MCP_INSPECTOR_TEMPLATE: &str = r#"#!/usr/bin/env bash

set -euo pipefail

pushd server
cargo build
popd

npx @modelcontextprotocol/inspector ./server/target/debug/{{name}}-server
"#;
