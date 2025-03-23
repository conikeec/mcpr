use mcpr::error::MCPError;
use mcpr::transport::sse::SSEServerTransport;
use mcpr::transport::Transport;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;
use tokio::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    id: u32,
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), MCPError> {
    // Create a SSE transport in server mode
    let uri = "http://127.0.0.1:8888";
    println!("Starting SSE server at {}", uri);

    // Create the transport in server mode
    let mut transport = SSEServerTransport::new(uri)?;

    // Start the server
    println!("Starting SSE server...");
    transport.start().await?;
    println!("SSE server started successfully!");
    println!("Endpoints:");
    println!("  - GET  {}/events    (SSE events stream)", uri);
    println!("  - POST {}/messages  (Message endpoint)", uri);
    println!("\nConnect with:");
    println!("  cargo run --example sse_client");

    // Create a shutdown signal
    let shutdown = Arc::new(Notify::new());
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let shutdown_flag_clone = shutdown_flag.clone();

    // Handle Ctrl+C to shutdown gracefully
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("Received Ctrl+C, shutting down...");
        shutdown_flag_clone.store(true, Ordering::SeqCst);
        shutdown_clone.notify_one();
    });

    // Send heartbeat messages periodically
    let heartbeat_task = tokio::spawn(async move {
        let mut counter = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            counter += 1;

            let heartbeat = Message {
                id: counter,
                jsonrpc: "2.0".to_string(),
                method: "heartbeat".to_string(),
                params: serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "count": counter
                }),
            };

            if let Err(e) = transport.send(&heartbeat).await {
                eprintln!("Error broadcasting heartbeat: {}", e);
            } else {
                println!("Sent heartbeat #{}", counter);
            }

            // Break if shutdown signal received
            if shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
        }

        // Close the transport when done
        if let Err(e) = transport.close().await {
            eprintln!("Error closing transport: {}", e);
        }

        Ok::<_, MCPError>(())
    });

    // Wait for shutdown signal
    shutdown.notified().await;

    // Wait for heartbeat task to finish
    if let Err(e) = heartbeat_task.await {
        eprintln!("Error waiting for heartbeat task: {}", e);
    }

    println!("Server shut down gracefully");
    Ok(())
}
