//! Templates for generating MCP server and client stubs with SSE transport

/// Template for project server main.rs with SSE transport
pub const PROJECT_SERVER_TEMPLATE: &str = r#"//! MCP Server for {{name}} project with SSE transport

use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::{sse::{Event, Sse}, IntoResponse},
    routing::{get, post},
    Router,
};
use clap::Parser;
use futures_util::stream::{self, Stream};
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema, Resource, Prompt, Role, PromptMessage},
    transport::{sse::SSETransport, Transport},
};
use mcpr_macros::{mcp_server, mcp_transport, mcp_prompt, mcp_resource, prompt, resource, tool};
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,
}

/// Define our transport type
#[mcp_transport]
struct ServerTransport {
    underlying: SSETransport,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
    on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl ServerTransport {
    fn new() -> Self {
        Self {
            underlying: SSETransport::new(),
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

/// Create our Axum app state
#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    server: Arc<MCPServerImpl>,
}

/// SSE Stream handler
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    
    info!("Client connected to SSE stream");
    
    // Create the SSE stream
    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(message) => {
                debug!("Sending SSE event: {}", message);
                let event = Event::default().data(message);
                Some((Ok(event), rx))
            }
            Err(e) => {
                error!("Error receiving message: {}", e);
                // Just return a keep-alive comment on error
                let event = Event::default().comment("keep-alive");
                Some((Ok(event), rx))
            }
        }
    });
    
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
    )
}

/// Post-message handler
async fn post_message(
    State(state): State<AppState>,
    Json(message): Json<String>,
) -> impl IntoResponse {
    debug!("Received message: {}", message);
    
    // Broadcast the message to all connected clients
    if let Err(e) = state.tx.send(message) {
        error!("Error broadcasting message: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    
    StatusCode::OK
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

    // Create the broadcast channel
    let (tx, _) = broadcast::channel(100);
    
    // Create our server implementation
    let server = Arc::new(MCPServerImpl::new());
    
    // Create app state
    let state = AppState { tx: tx.clone(), server };
    
    // Create the router
    let app = Router::new()
        .route("/events", get(sse_handler))
        .route("/message", post(post_message))
        .with_state(state);
    
    // Bind to the address
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    info!("Listening on {}", addr);
    
    // Start the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}"#;

/// Template for project client main.rs with SSE transport
pub const PROJECT_CLIENT_TEMPLATE: &str = r#"//! MCP Client for {{name}} project with SSE transport

use clap::Parser;
use eventsource_stream::Eventsource;
use futures_util::{StreamExt, SinkExt};
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema, Role, PromptMessage},
    transport::{
        sse::SSETransport,
        Transport,
    },
};
use mcpr_macros::{mcp_client, mcp_transport};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;
use tokio_stream::wrappers::LinesStream;
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
    
    /// Server URI
    #[arg(short, long, default_value = "http://localhost:3000")]
    uri: String,
    
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
    uri: String,
    http_client: HttpClient,
    event_stream: Option<tokio_stream::wrappers::Receiver<String>>,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
    on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl ClientTransport {
    fn new(uri: String) -> Self {
        Self {
            uri,
            http_client: HttpClient::new(),
            event_stream: None,
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }
}

impl Transport for ClientTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        debug!("Starting SSE transport with URI: {}", self.uri);
        
        // Create a channel for events
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        self.event_stream = Some(tokio_stream::wrappers::ReceiverStream::new(rx).into());
        
        // Clone values for the async task
        let uri = format!("{}/events", self.uri);
        let http_client = self.http_client.clone();
        
        // Spawn a task to listen for SSE events
        tokio::spawn(async move {
            loop {
                match http_client.get(&uri).send().await {
                    Ok(response) => {
                        let event_stream = response.bytes_stream().eventsource();
                        let mut event_stream = Box::pin(event_stream);
                        
                        while let Some(event_result) = event_stream.next().await {
                            match event_result {
                                Ok(event) => {
                                    if let Some(data) = event.data {
                                        if tx.send(data).await.is_err() {
                                            break;
                                        }
                                    }
                                },
                                Err(e) => {
                                    debug!("SSE stream error: {}", e);
                                    break;
                                }
                            }
                        }
                    },
                    Err(e) => {
                        debug!("Error connecting to SSE endpoint: {}", e);
                    }
                }
                
                // Wait before reconnecting
                sleep(Duration::from_secs(2)).await;
            }
        });
        
        Ok(())
    }

    fn close(&mut self) -> Result<(), MCPError> {
        if let Some(ref callback) = self.on_close {
            callback();
        }
        
        // Drop the event stream
        self.event_stream = None;
        
        Ok(())
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
        // Send the message to the server using HTTP POST
        let uri = format!("{}/message", self.uri);
        
        // We need to use tokio::runtime::Handle to run this blocking code
        let response = self.http_client.post(&uri)
            .header("Content-Type", "application/json")
            .body(message.to_string())
            .send();
            
        // This is a simplification for the example
        // In a real application, we'd handle this properly with tokio
        match tokio::runtime::Handle::current().block_on(response) {
            Ok(_) => Ok(()),
            Err(e) => Err(MCPError::Transport(format!("Failed to send message: {}", e))),
        }
    }

    fn receive(&mut self) -> Result<String, MCPError> {
        // Get the event stream
        let event_stream = self.event_stream.as_mut()
            .ok_or_else(|| MCPError::Transport("Transport not started".to_string()))?;
            
        // Wait for a message
        let message = tokio::runtime::Handle::current().block_on(async {
            match event_stream.recv().await {
                Some(msg) => Ok(msg),
                None => Err(MCPError::Transport("Event stream closed".to_string())),
            }
        })?;
        
        // Process the message
        if let Some(ref callback) = self.on_message {
            callback(&message);
        }
        
        Ok(message)
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

/// Interactive client session
async fn run_interactive_session(client: &MCPClientImpl) -> Result<(), Box<dyn Error>> {
    println!("Interactive MCP Client");
    println!("Type 'exit' to quit");
    println!("Available commands: hello, calculate");
    
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let line = match reader.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
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
                    match reader.next_line().await {
                        Ok(Some(input)) => input,
                        _ => "World".to_string(),
                    }
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
    
    info!("Starting {{name}} client...");
    
    // Create transport and client
    let transport = ClientTransport::new(args.uri);
    let mut client = MCPClientImpl::new(transport);
    
    // Connect to server
    info!("Connecting to server at {}...", args.uri);
    client.connect()?;
    
    // Process commands
    if args.interactive {
        run_interactive_session(&client).await?;
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

/// Template for project server Cargo.toml with SSE transport
pub const PROJECT_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"
authors = ["Generated with mcpr-cli"]
description = "MCP Server for {{name}} project using SSE transport"

[dependencies]
mcpr = "{{version}}"
mcpr-macros = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
axum = "0.7"
tokio = { version = "1.33", features = ["full"] }
"#;

/// Template for project client Cargo.toml with SSE transport
pub const PROJECT_CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-client"
version = "0.1.0"
edition = "2021"
authors = ["Generated with mcpr-cli"]
description = "MCP Client for {{name}} project using SSE transport"

[dependencies]
mcpr = "{{version}}"
mcpr-macros = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
reqwest = { version = "0.11", features = ["json", "stream"] }
tokio = { version = "1.33", features = ["full"] }
tokio-stream = "0.1"
eventsource-stream = "0.2"
futures-util = "0.3"
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
