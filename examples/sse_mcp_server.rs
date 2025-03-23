use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema},
    server::{Server, ServerConfig},
    transport::sse::SSEServerTransport,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Notify;

#[tokio::main]
async fn main() -> Result<(), MCPError> {
    // Initialize logging (optional)
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    // Create a transport for SSE server (listens on all interfaces)
    let uri = "http://127.0.0.1:8889";
    let transport = SSEServerTransport::new(uri)?;

    // Create an echo tool
    let echo_tool = Tool {
        name: "echo".to_string(),
        description: Some("Echoes back the input".to_string()),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(
                [(
                    "message".to_string(),
                    json!({
                        "type": "string",
                        "description": "The message to echo"
                    }),
                )]
                .into_iter()
                .collect::<HashMap<_, _>>(),
            ),
            required: Some(vec!["message".to_string()]),
        },
    };

    // Configure the server
    let server_config = ServerConfig::new()
        .with_name("SSE MCP Server")
        .with_version("1.0.0")
        .with_tool(echo_tool);

    // Create the server
    let mut server = Server::new(server_config);

    // Register the echo tool handler
    server.register_tool_handler("echo", |params| async move {
        // Extract the message parameter
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::Protocol("Missing message parameter".to_string()))?;

        println!("Echo request: {}", message);

        // Return the message as the result
        Ok(json!({
            "result": message
        }))
    })?;

    // Create a shutdown signal
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();

    // Handle Ctrl+C
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            println!("Received Ctrl+C, shutting down...");
            shutdown_clone.notify_one();
        }
    });

    // Start the server in the background with SSE transport
    server.start_background(transport).await?;

    println!("Server started on {}. Press Ctrl+C to stop.", uri);
    println!("Endpoints:");
    println!("  - GET  {}/events    (SSE events stream)", uri);
    println!("  - POST {}/messages  (Message endpoint)", uri);
    println!("\nTest with curl:");
    println!("  curl -N -H \"Accept: text/event-stream\" {}/events", uri);
    println!("  curl -X POST -H \"Content-Type: application/json\" -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}}' {}/messages", uri);

    // Wait for shutdown signal
    shutdown.notified().await;

    println!("Server shut down gracefully");
    Ok(())
}
