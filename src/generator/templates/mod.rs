//! Templates for generating MCP server and client stubs

pub mod sse;
pub mod stdio;
// WebSocket templates will be implemented in a future release

// Re-export all templates from transport modules
pub use sse::{
    PROJECT_CLIENT_CARGO_TEMPLATE as SSE_CLIENT_CARGO_TEMPLATE,
    PROJECT_CLIENT_TEMPLATE as SSE_CLIENT_TEMPLATE,
    PROJECT_SERVER_CARGO_TEMPLATE as SSE_SERVER_CARGO_TEMPLATE,
    PROJECT_SERVER_TEMPLATE as SSE_SERVER_TEMPLATE,
    PROJECT_TEST_SCRIPT_TEMPLATE as SSE_TEST_SCRIPT_TEMPLATE,
};

// TODO: Remove this comment after confirming everything works
// The SSE client templates are now imported from sse.rs module

pub use stdio::{
    PROJECT_CLIENT_CARGO_TEMPLATE as STDIO_CLIENT_CARGO_TEMPLATE,
    PROJECT_CLIENT_TEMPLATE as STDIO_CLIENT_TEMPLATE,
    PROJECT_SERVER_TEMPLATE as STDIO_SERVER_TEMPLATE,
    PROJECT_TEST_SCRIPT_TEMPLATE as STDIO_TEST_SCRIPT_TEMPLATE,
};

/// Template for stdio server Cargo.toml
pub const STDIO_SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}-server"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
# For local development, use path dependency:
# mcpr = { path = "../.." }
# For production, use version from crates.io:
mcpr = "0.2.3"
clap = { version = "4.0", features = ["derive"] }
serde = "1.0"
serde_json = "1.0"
env_logger = "0.10"
log = "0.4"
anyhow = "1.0"
thiserror = "1.0"
tokio = { version = "1", features = ["full"] }
"#;

// WebSocket templates will be added when the WebSocket transport is implemented

/// Template for server main.rs
pub const SERVER_MAIN_TEMPLATE: &str = r#"//! MCP Server: {{name}}

use clap::Parser;
use mcpr::schema::{
    CallToolParams, CallToolResult, Implementation, InitializeResult, JSONRPCError, JSONRPCMessage,
    JSONRPCResponse, ServerCapabilities, TextContent, Tool, ToolInputSchema, ToolResultContent,
    ToolsCapability,
};
use mcpr::transport::stdio::StdioTransport;
use serde_json::{json, Value};
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

    // Create a transport
    let transport = StdioTransport::new();

    // Create a server
    let mut server = mcpr::server::Server::new(
        mcpr::server::ServerConfig::new()
            .with_name("{{name}}")
            .with_version("0.1.0")
            .with_tool(Tool {
                name: "hello".to_string(),
                description: "A simple tool that greets a person by name".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name of the person to greet"
                        }
                    },
                    "required": ["name"]
                }),
            }),
    );

    // Register tool handlers
    server.register_tool_handler("hello", |params| {
        let name = params["name"].as_str().unwrap_or("World");
        Ok(json!({
            "message": format!("Hello, {}!", name)
        }))
    })?;

    // Start the server
    info!("Starting MCP server");
    server.start(transport)?;

    Ok(())
}

/// Template for server Cargo.toml
pub const SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
description = "MCP server generated using mcpr CLI"

[dependencies]
mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
"#;

/// Template for client main.rs
pub const CLIENT_MAIN_TEMPLATE: &str = r#"//! MCP Client: {{name}}

use clap::Parser;
use mcpr::client::Client;
use mcpr::transport::stdio::StdioTransport;
use serde_json::{json, Value};
use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use log::{info, error, debug, warn};

/// CLI arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,

    /// Run in interactive mode
    #[arg(short, long)]
    interactive: bool,

    /// Name to greet (for non-interactive mode)
    #[arg(short, long)]
    name: Option<String>,

    /// Connect to an existing server
    #[arg(short, long)]
    connect: bool,

    /// Server command to run
    #[arg(long, default_value = "../server/target/debug/{{name}}")]
    server_cmd: String,
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

    // Create a transport
    let transport = if args.connect {
        info!("Connecting to existing server");
        StdioTransport::new()
    } else {
        info!("Starting server process: {}", args.server_cmd);
        let server_process = Command::new(&args.server_cmd)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()?;
        
        StdioTransport::from_process(server_process)
    };

    // Create a client
    let mut client = Client::new(transport);

    // Initialize the client
    info!("Initializing client");
    let init_result = client.initialize()?;
    info!("Connected to server: {} v{}", init_result.name, init_result.version);
    
    if let Some(tools) = init_result.capabilities.tools {
        info!("Available tools:");
        for tool in tools.tools {
            info!("  - {}: {}", tool.name, tool.description);
        }
    }

    if args.interactive {
        // Interactive mode
        loop {
            print!("Enter tool name (or 'exit' to quit): ");
            io::stdout().flush()?;
            
            let mut tool_name = String::new();
            io::stdin().read_line(&mut tool_name)?;
            let tool_name = tool_name.trim();
            
            if tool_name == "exit" {
                break;
            }
            
            if tool_name == "hello" {
                print!("Enter name to greet: ");
                io::stdout().flush()?;
                
                let mut name = String::new();
                io::stdin().read_line(&mut name)?;
                let name = name.trim();
                
                let result: Value = client.call_tool(tool_name, &json!({
                    "name": name
                }))?;
                
                println!("Result: {}", result["message"]);
            } else {
                println!("Unknown tool: {}", tool_name);
            }
        }
    } else if let Some(name) = args.name {
        // One-shot mode
        let result: Value = client.call_tool("hello", &json!({
            "name": name
        }))?;
        
        println!("{}", result["message"]);
    } else {
        println!("Please specify a name with --name or use --interactive mode");
    }

    // Shutdown the client
    info!("Shutting down client");
    client.shutdown()?;

    Ok(())
}

/// Template for client Cargo.toml
pub const CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
description = "MCP client generated using mcpr CLI"

[dependencies]
mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
"#;

/// Template for server README.md
pub const SERVER_README_TEMPLATE: &str = r#"# {{name}}

This is a server implementation for the {{name}} MCP project.

## Building and Running

```bash
cargo build
cargo run -- --help
```

## Features

- Generated with mcpr CLI
"#;

/// Template for client README.md
pub const CLIENT_README_TEMPLATE: &str = r#"# {{name}} Client

This is a client implementation for the {{name}} MCP project.

## Building and Running

```bash
cargo build
cargo run -- --help
```

## Features

- Generated with mcpr CLI
"#;

/// Default template for server Cargo.toml
pub const SERVER_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
description = "MCP server generated using mcpr CLI"

[dependencies]
mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
"#;

/// Default template for client Cargo.toml
pub const CLIENT_CARGO_TEMPLATE: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
description = "MCP client generated using mcpr CLI"

[dependencies]
mcpr = "{{version}}"
clap = { version = "4.4", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
"#;

pub const PROJECT_README_SSE_TEMPLATE: &str = r#"# {{name}} - MCP Project

A complete MCP project with both client and server components, using SSE transport.

## Features

- **Server-Sent Events Transport**: HTTP-based transport with proper client registration and message handling
- **Multiple Client Support**: Server can handle multiple clients simultaneously
- **Interactive Mode**: Choose tools and provide parameters interactively
- **One-shot Mode**: Run queries directly from the command line
- **Comprehensive Logging**: Detailed logging for debugging and monitoring

## Project Structure

- `client/`: The MCP client implementation
- `server/`: The MCP server implementation with tools
- `test.sh`: A test script to run both client and server

## Building

```bash
# Build the server
cd server
cargo build

# Build the client
cd ../client
cargo build
```

## Running

### Start the Server

```bash
cd server
cargo run -- --port 8080
```

### Run the Client

```bash
cd client
cargo run -- --uri "http://localhost:8080" --name "Your Name"
```

### Interactive Mode

```bash
cd client
cargo run -- --uri "http://localhost:8080" --interactive
```

## Testing

```bash
./test.sh
```

## Available Tools

- `hello`: A simple tool that greets a person by name
"#;

pub const PROJECT_README_STDIO_TEMPLATE: &str = r#"# {{name}} - MCP Project

A complete MCP project with both client and server components, using stdio transport.

## Features

- **Robust Communication**: Reliable stdio transport with proper error handling and timeout management
- **Multiple Connection Methods**: Connect to an already running server or start a new server process
- **Interactive Mode**: Choose tools and provide parameters interactively
- **One-shot Mode**: Run queries directly from the command line
- **Comprehensive Logging**: Detailed logging for debugging and monitoring

## Project Structure

- `client/`: The MCP client implementation
- `server/`: The MCP server implementation with tools
- `test.sh`: A test script to run both client and server

## Building

```bash
# Build the server
cd server
cargo build

# Build the client
cd ../client
cargo build
```

## Running

### Start the Server

```bash
cd server
cargo run
```

### Run the Client

```bash
cd client
cargo run -- --name "Your Name"
```

### Interactive Mode

```bash
cd client
cargo run -- --interactive
```

## Testing

```bash
./test.sh
```

## Available Tools

- `hello`: A simple tool that greets a person by name
"#;
