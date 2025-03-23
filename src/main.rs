//! MCP CLI tool for generating server and client stubs

use clap::{Parser, Subcommand};
use env_logger;
use log::{error, info, warn};
use mcpr::{
    client::Client,
    error::MCPError,
    schema::common::Tool,
    server::{Server, ServerConfig},
    transport::{
        sse::{SSEClientTransport, SSEServerTransport},
        stdio::StdioTransport,
        Transport,
    },
};
use std::path::PathBuf;

/// MCP CLI tool for generating server and client stubs
#[derive(Parser, Debug)]
#[command(name = "mcpr", about = "MCP CLI tools", version)]
enum Cli {
    /// Generate a server stub
    GenerateServer {
        /// Server name
        #[arg(short, long)]
        name: String,

        /// Transport type to use (stdio, sse)
        #[arg(short, long, default_value = "stdio")]
        transport: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
    },

    /// Generate a client stub
    GenerateClient {
        /// Client name
        #[arg(short, long)]
        name: String,

        /// Transport type to use (stdio, sse)
        #[arg(short, long, default_value = "stdio")]
        transport: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
    },

    /// Generate a complete MCP project with client and server
    GenerateProject {
        /// Project name
        #[arg(short, long)]
        name: String,

        /// Transport type to use (stdio, sse)
        #[arg(short, long, default_value = "stdio")]
        transport: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
    },

    /// Run a server
    RunServer {
        /// Port to listen on
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Transport type to use (stdio, sse)
        #[arg(short, long, default_value = "stdio")]
        transport: String,

        /// Enable debug mode
        #[arg(long)]
        debug: bool,
    },

    /// Connect to a server as a client
    Connect {
        /// URI of the server to connect to
        #[arg(short, long)]
        uri: String,

        /// Run in interactive mode
        #[arg(short, long)]
        interactive: bool,

        /// Name to use for greeting
        #[arg(short, long, default_value = "Default User")]
        name: String,

        /// Transport type to use (stdio, sse)
        #[arg(short, long)]
        transport: String,

        /// Operation to perform (interactive, repl, tool_name)
        #[arg(short, long)]
        operation: Option<String>,

        /// Parameters for the operation
        #[arg(short, long)]
        params: Option<String>,
    },

    /// Validate an MCP message
    Validate {
        /// Path to the message file
        #[arg(short, long)]
        path: String,
    },
}

/// Connect command parameters
#[derive(Debug, Clone)]
struct Connect {
    uri: String,
    interactive: bool,
    name: String,
    transport: String,
    operation: Option<String>,
    params: Option<String>,
}

/// Main entry point of the CLI application
#[tokio::main]
async fn main() -> Result<(), MCPError> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    // Parse command-line arguments
    let cli = Cli::parse();

    match cli {
        Cli::GenerateServer {
            name,
            transport,
            output,
        } => {
            info!(
                "Generating server stub with name '{}' to '{}'",
                name, output
            );
            let output_path = PathBuf::from(output.clone());

            // Generate server using the generator module
            mcpr::generator::generate_server(&name, &output_path)
                .map_err(|e| MCPError::Transport(format!("Failed to generate server: {}", e)))
        }
        Cli::GenerateClient {
            name,
            transport,
            output,
        } => {
            info!(
                "Generating client stub with name '{}' to '{}'",
                name, output
            );
            let output_path = PathBuf::from(output.clone());

            // Generate client using the generator module
            mcpr::generator::generate_client(&name, &output_path)
                .map_err(|e| MCPError::Transport(format!("Failed to generate client: {}", e)))
        }
        Cli::GenerateProject {
            name,
            transport,
            output,
        } => {
            info!(
                "Generating project '{}' in '{}' with transport '{}'",
                name, output, transport
            );

            // Generate project using the generator module
            mcpr::generator::generate_project(&name, &output, &transport)
                .map_err(|e| MCPError::Transport(format!("Failed to generate project: {}", e)))
        }
        Cli::RunServer {
            port,
            transport,
            debug,
        } => run_server(port, &transport, debug).await,
        Cli::Connect {
            uri,
            interactive,
            name,
            transport,
            operation,
            params,
        } => {
            run_client(Connect {
                uri,
                interactive,
                name,
                transport,
                operation,
                params,
            })
            .await
        }
        Cli::Validate { path } => {
            info!("Validating message from '{}'", path);
            // TODO: Implement message validation
            info!("Message validation not yet implemented");
            Err(MCPError::UnsupportedFeature(
                "Message validation not yet implemented".to_string(),
            ))
        }
    }
}

/// Run the server with the specified configuration
async fn run_server(port: u16, transport_type: &str, debug: bool) -> Result<(), MCPError> {
    info!(
        "Running server on port {} with transport type {}",
        port, transport_type
    );

    if debug {
        info!("Debug mode enabled");
    }

    // Depending on the transport type, start the appropriate server
    match transport_type {
        "stdio" => {
            info!("Starting server with stdio transport");
            // TODO: Implement stdio server
            Err(MCPError::UnsupportedFeature(
                "Stdio server not yet implemented".to_string(),
            ))
        }
        "sse" => {
            info!("Starting server with SSE transport on port {}", port);

            // Create a URI for the SSE server
            let uri = format!("http://0.0.0.0:{}", port);

            // Create the SSE transport
            let transport = SSEServerTransport::new(&uri)?;

            // Configure a basic echo tool
            let echo_tool = Tool {
                name: "echo".to_string(),
                description: Some("Echo tool".to_string()),
                input_schema: mcpr::schema::common::ToolInputSchema {
                    r#type: "object".to_string(),
                    properties: Some(
                        [(
                            "message".to_string(),
                            serde_json::json!({
                                "type": "string",
                                "description": "Message to echo"
                            }),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                    required: Some(vec!["message".to_string()]),
                },
            };

            // Create server config
            let server_config = ServerConfig::new()
                .with_name("MCP SSE Server")
                .with_version("1.0.0")
                .with_tool(echo_tool);

            // Create and start the server
            let mut server = Server::new(server_config);

            // Register the echo tool handler
            server.register_tool_handler("echo", |params| async move {
                let message = params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MCPError::Protocol("Missing message parameter".to_string()))?;

                info!("Echo request: {}", message);

                Ok(serde_json::json!({"result": message}))
            })?;

            // Start the server
            info!("Starting SSE server on {}", uri);
            server.serve(transport).await
        }
        _ => {
            error!("Unsupported transport type: {}", transport_type);
            Err(MCPError::Transport(format!(
                "Unsupported transport type: {}",
                transport_type
            )))
        }
    }
}

/// Run the client with the specified configuration
async fn run_client(cmd: Connect) -> Result<(), MCPError> {
    info!("Connecting to {}", cmd.uri);

    // Handle different transport types directly
    match cmd.transport.as_str() {
        "sse" => {
            info!("Using SSE transport with URI: {}", cmd.uri);
            // For SSE transport, we now only need to provide the events URL
            // The messages URL will be dynamically received from the server via an "endpoint" event
            let transport = SSEClientTransport::new(&cmd.uri)
                .map_err(|e| MCPError::Transport(format!("Failed to create SSE client: {}", e)))?;
            let mut client = Client::new(transport);
            handle_client_session(&mut client, cmd).await
        }
        "stdio" => {
            info!("Using stdio transport");
            let transport = StdioTransport::new();
            let mut client = Client::new(transport);
            handle_client_session(&mut client, cmd).await
        }
        _ => Err(MCPError::Transport(format!(
            "Unsupported transport type: {}",
            cmd.transport
        ))),
    }
}

// Helper function to handle the client session logic
async fn handle_client_session<T: Transport + Send + Sync>(
    client: &mut Client<T>,
    cmd: Connect,
) -> Result<(), MCPError> {
    // Initialize the client
    client.initialize().await?;

    // Display server information if available
    info!("Connected to server");

    // Get available tools by calling the list_tools method
    let tools_result = client
        .call_tool::<_, serde_json::Value>("list_tools", &serde_json::Value::Null)
        .await;

    let _tools = match tools_result {
        Ok(response) => {
            if let Some(tools_array) = response.as_array() {
                info!("Available tools:");
                for tool in tools_array {
                    if let (Some(name), Some(description)) = (
                        tool.get("name").and_then(|n| n.as_str()),
                        tool.get("description").and_then(|d| d.as_str()),
                    ) {
                        info!(" - {}: {}", name, description);
                    }
                }
                tools_array.to_vec()
            } else {
                info!("No tools available");
                Vec::new()
            }
        }
        Err(e) => {
            warn!("Could not retrieve tools: {}", e);
            Vec::new()
        }
    };

    // Handle requested operations
    match cmd.operation.as_deref() {
        Some("interactive") => {
            info!("Starting interactive session");
            // Interactive session logic here
            Err(MCPError::UnsupportedFeature(
                "Interactive session not implemented yet".to_string(),
            ))
        }
        Some("repl") => {
            info!("Starting REPL");
            // REPL session logic here
            Err(MCPError::UnsupportedFeature(
                "REPL not implemented yet".to_string(),
            ))
        }
        Some(tool_name) => {
            info!("Calling tool: {}", tool_name);

            // Parse parameters
            let params = if let Some(params_str) = cmd.params.as_ref() {
                serde_json::from_str(params_str)
                    .map_err(|e| MCPError::Transport(format!("Invalid parameters: {}", e)))?
            } else {
                serde_json::Value::Null
            };

            // Call the requested tool
            match client
                .call_tool::<_, serde_json::Value>(tool_name, &params)
                .await
            {
                Ok(response) => {
                    println!("{}", serde_json::to_string_pretty(&response).unwrap());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        None => {
            // Default to the hello tool if no operation is specified
            info!("No operation specified, using hello tool");

            // Use empty params for hello
            let params = serde_json::Value::Null;

            // Call the hello tool
            let response: serde_json::Value = client.call_tool("hello", &params).await?;

            println!("{}", serde_json::to_string_pretty(&response).unwrap());
            Ok(())
        }
    }?;

    // Shut down the client
    client.shutdown().await?;

    Ok(())
}
