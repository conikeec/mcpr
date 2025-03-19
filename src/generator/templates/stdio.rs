//! Templates for generating MCP server and client stubs with stdio transport

/// Template for project server main.rs with stdio transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"//! MCP Server for {{name}} project with stdio transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema, Resource, Prompt, Role, PromptMessage},
    transport::{
        stdio::StdioTransport,
        Transport,
    },
};
use mcpr_macros::{mcp_server, mcp_transport, mcp_prompt, mcp_resource, prompt, resource, tool};
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

/// Define our transport type
#[mcp_transport]
struct ServerTransport {
    underlying: StdioTransport,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
    on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl ServerTransport {
    fn new() -> Self {
        Self {
            underlying: StdioTransport::new(),
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }
}

/// Implement the Transport trait manually (normally done by macro)
impl Transport for ServerTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        self.underlying.start()
    }

    fn close(&mut self) -> Result<(), MCPError> {
        if let Some(ref callback) = self.on_close {
            callback();
        }
        self.underlying.close()
    }

    fn set_on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        self.on_close = Some(callback);
    }

    fn set_on_error(&mut self, callback: Box<dyn Fn(&MCPError) + Send + Sync>) {
        self.on_error = Some(callback);
    }

    fn set_on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
        self.on_message = Some(callback);
    }

    fn send(&mut self, message: &str) -> Result<(), MCPError> {
        self.underlying.send(message)
    }

    fn receive(&mut self) -> Result<String, MCPError> {
        let msg = self.underlying.receive()?;
        if let Some(ref callback) = self.on_message {
            callback(&msg);
        }
        Ok(msg)
    }
}

/// Define prompts for the server
#[mcp_prompt]
struct ServerPromptProvider;

impl ServerPromptProvider {
    fn new() -> Self {
        Self { }
    }

    #[prompt]
    fn system_prompt(&self, context: String) -> Result<String, MCPError> {
        Ok(format!("You are a helpful assistant with the following context: {}", context))
    }

    #[prompt]
    fn user_greeting(&self, name: String, formal: bool) -> Result<String, MCPError> {
        if formal {
            Ok(format!("Good day, Mr./Ms. {}", name))
        } else {
            Ok(format!("Hey {}!", name))
        }
    }
}

/// Define resources for the server
#[mcp_resource]
struct ServerResourceProvider;

impl ServerResourceProvider {
    fn new() -> Self {
        Self { }
    }

    #[resource]
    fn get_user_info(&self, user_id: String) -> Result<Value, MCPError> {
        // In a real application, this would fetch from a database
        Ok(serde_json::json!({
            "id": user_id,
            "name": "Test User",
            "email": "user@example.com"
        }))
    }

    #[resource]
    fn get_product_details(&self, product_id: String, include_pricing: bool) -> Result<Value, MCPError> {
        let mut product = serde_json::json!({
            "id": product_id,
            "name": "Test Product",
            "description": "This is a test product"
        });
        
        if include_pricing {
            if let Value::Object(ref mut map) = product {
                map.insert("price".to_string(), serde_json::json!(99.99));
                map.insert("currency".to_string(), serde_json::json!("USD"));
            }
        }
        
        Ok(product)
    }
}

/// Define our MCP server
#[mcp_server]
struct MCPServerImpl {
    prompt_provider: ServerPromptProvider,
    resource_provider: ServerResourceProvider,
    name: String,
    version: String,
}

impl MCPServerImpl {
    fn new() -> Self {
        Self {
            prompt_provider: ServerPromptProvider::new(),
            resource_provider: ServerResourceProvider::new(),
            name: "{{name}} Server".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    #[tool]
    fn hello(&self, name: String) -> Result<Value, MCPError> {
        Ok(serde_json::json!({
            "message": format!("Hello, {}!", name)
        }))
    }
    
    #[tool]
    fn calculate(&self, a: f64, b: f64, operation: String) -> Result<Value, MCPError> {
        let result = match operation.as_str() {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(MCPError::Protocol("Cannot divide by zero".to_string()));
                }
                a / b
            },
            _ => return Err(MCPError::Protocol(format!("Unknown operation: {}", operation))),
        };
        
        Ok(serde_json::json!({
            "result": result
        }))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize logging
    if args.debug {
        std::env::set_var("RUST_LOG", "debug,mcpr=debug");
    } else {
        std::env::set_var("RUST_LOG", "info,mcpr=info");
    }
    env_logger::init();

    // Create our server implementation
    let server = MCPServerImpl::new();
    
    // Create the transport
    let transport = ServerTransport::new();
    
    info!("Starting {{name}} server...");
    
    // In a real application, you would use the start method on the server
    // This is just a simplified example for demonstration
    // Normally the mcp_server macro would generate this method
    // server.start(transport)?;
    
    // For now, we'll just simulate the server running
    let mut transport = transport;
    transport.start()?;
    
    println!("Server is running. Press Ctrl+C to exit.");
    
    // In a real application, we'd have a proper event loop here
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}"#;

/// Template for project client main.rs with stdio transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with stdio transport

use clap::Parser;
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema, Role, PromptMessage},
    transport::{
        stdio::StdioTransport,
        Transport,
    },
};
use mcpr_macros::{mcp_client, mcp_transport};
use serde::{Deserialize, Serialize};
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

/// Define our transport type
#[mcp_transport]
struct ClientTransport {
    underlying: StdioTransport,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
    on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl ClientTransport {
    fn new(process: Option<Child>) -> Self {
        Self {
            underlying: match process {
                Some(proc) => StdioTransport::from_process(proc),
                None => StdioTransport::new(),
            },
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }
}

impl Transport for ClientTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        self.underlying.start()
    }

    fn close(&mut self) -> Result<(), MCPError> {
        if let Some(ref callback) = self.on_close {
            callback();
        }
        self.underlying.close()
    }

    fn set_on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        self.on_close = Some(callback);
    }

    fn set_on_error(&mut self, callback: Box<dyn Fn(&MCPError) + Send + Sync>) {
        self.on_error = Some(callback);
    }

    fn set_on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
        self.on_message = Some(callback);
    }

    fn send(&mut self, message: &str) -> Result<(), MCPError> {
        self.underlying.send(message)
    }

    fn receive(&mut self) -> Result<String, MCPError> {
        let msg = self.underlying.receive()?;
        if let Some(ref callback) = self.on_message {
            callback(&msg);
        }
        Ok(msg)
    }
}

/// Define the client trait with required methods
trait MCPClient {
    fn hello(&self, name: String) -> Result<HelloResponse, MCPError>;
    fn calculate(&self, a: f64, b: f64, operation: String) -> Result<CalculateResponse, MCPError>;
}

/// Define response structures for our client
#[derive(Debug, Deserialize)]
struct HelloResponse {
    message: String,
}

#[derive(Debug, Deserialize)]
struct CalculateResponse {
    result: f64,
}

/// Define our client implementation
#[mcp_client]
struct MCPClientImpl {
    transport: ClientTransport,
}

impl MCPClientImpl {
    fn new(transport: ClientTransport) -> Self {
        Self { transport }
    }

    fn connect(&mut self) -> Result<(), MCPError> {
        self.transport.start()?;
        debug!("Transport started");
        
        // Initialize would go here
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), MCPError> {
        self.transport.close()?;
        debug!("Transport closed");
        Ok(())
    }
}

/// Start a new server and connect to it
fn start_server(server_cmd: &str) -> Result<Child, Box<dyn Error>> {
    info!("Starting server process: {}", server_cmd);
    
    let server_process = Command::new(server_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Redirect stderr to our process for debugging
        .spawn()?;
    
    // Give the server a moment to start up
    thread::sleep(Duration::from_millis(500));
    
    Ok(server_process)
}

/// Interactive client session
fn run_interactive_session(client: &MCPClientImpl) -> Result<(), Box<dyn Error>> {
    println!("Interactive MCP Client");
    println!("Type 'exit' to quit");
    println!("Available commands: hello, calculate");
    
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let line = match lines.next() {
            Some(line) => line?,
            None => break,
        };
        
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        
        match parts[0] {
            "exit" => break,
            "hello" => {
                let name = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    print!("Enter name: ");
    io::stdout().flush()?;
                    lines.next().unwrap_or(Ok("World".to_string()))?
                };
                
                match client.hello(name) {
                    Ok(response) => println!("{}", response.message),
                    Err(e) => println!("Error: {}", e),
                }
            },
            "calculate" => {
                if parts.len() < 4 {
                    println!("Usage: calculate <number> <operation> <number>");
                    println!("Operations: add, subtract, multiply, divide");
                    continue;
                }
                
                let a: f64 = match parts[1].parse() {
                    Ok(num) => num,
                    Err(_) => {
                        println!("Invalid first number");
                        continue;
                    }
                };
                
                let operation = parts[2].to_string();
                
                let b: f64 = match parts[3].parse() {
                    Ok(num) => num,
                    Err(_) => {
                        println!("Invalid second number");
                        continue;
                    }
                };
                
                match client.calculate(a, b, operation) {
                    Ok(response) => println!("Result: {}", response.result),
                    Err(e) => println!("Error: {}", e),
                }
            },
            _ => println!("Unknown command: {}", parts[0]),
        }
    }
    
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize logging
    if args.debug {
        std::env::set_var("RUST_LOG", "debug,mcpr=debug");
    } else {
        std::env::set_var("RUST_LOG", "info,mcpr=info");
    }
    env_logger::init();
    
    info!("Starting {{name}} client...");
    
    // Start or connect to server
    let server_process = if args.connect {
        info!("Connecting to existing server");
        None
    } else {
        Some(start_server(&args.server_cmd)?)
    };
    
    // Create transport and client
    let transport = ClientTransport::new(server_process);
    let mut client = MCPClientImpl::new(transport);
    
    // Connect to server
    info!("Connecting to server...");
    client.connect()?;
    
    // Process commands
    if args.interactive {
        run_interactive_session(&client)?;
    } else if let Some(name) = &args.name {
        // Just run the hello command once with the provided name
        match client.hello(name.clone()) {
            Ok(response) => println!("{}", response.message),
            Err(e) => eprintln!("Error: {}", e),
        }
    } else {
        println!("Either use --interactive mode or provide a --name parameter");
    }
    
    // Disconnect from server
    info!("Disconnecting from server...");
    client.disconnect()?;
    
    Ok(())
}"#;

/// Template for project server Cargo.toml with stdio transport
pub const PROJECT_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"
authors = ["Generated with mcpr-cli"]
description = "MCP Server for {{name}} project using stdio transport"

[dependencies]
mcpr = "{{version}}"
mcpr-macros = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
"#;

/// Template for project client Cargo.toml with stdio transport
pub const PROJECT_CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-client"
version = "0.1.0"
edition = "2021"
authors = ["Generated with mcpr-cli"]
description = "MCP Client for {{name}} project using stdio transport"

[dependencies]
mcpr = "{{version}}"
mcpr-macros = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
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
