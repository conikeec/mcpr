use futures::future::join_all;
use log::{error, info};
use mcpr::{client::Client, error::MCPError, transport::sse::SSEClientTransport};
use serde_json::json;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), MCPError> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    // Connect to the SSE server
    info!("Connecting to SSE server...");
    let transport = SSEClientTransport::new("http://127.0.0.1:8889/events")?;

    // Create a client
    let mut client = Client::new(transport);

    // Initialize the client
    info!("Initializing client...");
    let init_result = client.initialize().await?;

    // Print server info
    if let Some(server_info) = init_result.get("serverInfo") {
        if let (Some(name), Some(version), Some(protocol_version)) = (
            server_info.get("name").and_then(|v| v.as_str()),
            server_info.get("version").and_then(|v| v.as_str()),
            init_result.get("protocolVersion").and_then(|v| v.as_str()),
        ) {
            info!(
                "Connected to server: {} v{} (protocol {})",
                name, version, protocol_version
            );
        }
    }

    // Make concurrent requests
    let num_requests = 10;
    info!("Making {} concurrent echo requests...", num_requests);

    let start = Instant::now();
    let mut tasks = Vec::with_capacity(num_requests);

    // Create a separate task for each request
    for i in 0..num_requests {
        let message = format!("Concurrent message {}", i);
        let params = json!({
            "message": message
        });

        // Spawn a separate task with its own client for each request
        let task_handle = tokio::spawn(async move {
            // Create a new client for this task
            let transport =
                SSEClientTransport::new("http://127.0.0.1:8889/events").map_err(|e| {
                    error!("Task {} - Failed to create transport: {}", i, e);
                    (i, format!("Transport error: {}", e))
                })?;
            let mut client = Client::new(transport);

            // Initialize the client
            if let Err(e) = client.initialize().await {
                error!("Task {} - Failed to initialize client: {}", i, e);
                return Err((i, format!("Init error: {}", e)));
            }

            // Call the tool
            match client.call_tool::<_, String>("echo", &params).await {
                Ok(result) => {
                    info!("Request {} result: {}", i, result);

                    // Clean shutdown
                    let _ = client.shutdown().await;

                    Ok((i, result))
                }
                Err(e) => {
                    error!("Request {} error: {}", i, e);

                    // Still try to shut down
                    let _ = client.shutdown().await;

                    Err((i, format!("Call error: {}", e)))
                }
            }
        });

        tasks.push(task_handle);
    }

    // Wait for all tasks to complete
    let results = join_all(tasks).await;
    let elapsed = start.elapsed();

    // Process results
    let mut successes = 0;
    let mut failures = 0;

    for result in &results {
        match result {
            Ok(Ok(_)) => successes += 1,
            Ok(Err(_)) | Err(_) => failures += 1,
        }
    }

    info!(
        "Completed {} requests in {:.2?} ({} successes, {} failures)",
        num_requests, elapsed, successes, failures
    );

    // Shutdown the main client
    info!("Shutting down client...");
    client.shutdown().await?;

    Ok(())
}
