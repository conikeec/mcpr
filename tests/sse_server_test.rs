use futures::{Stream, StreamExt};
use mcpr::{
    error::MCPError,
    schema::{
        common::{Tool, ToolInputSchema},
        json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    },
    server::{Server, ServerConfig},
    transport::sse::SSEServerTransport,
};
use reqwest::{header, Client as ReqwestClient};
use serde_json::{json, Value};
use std::{collections::HashMap, pin::Pin, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};
use tokio_util::io::StreamReader;

/// Starts a test server with an echo tool
async fn start_test_server() -> Result<(String, mpsc::Sender<()>), MCPError> {
    // Use a random port to avoid conflicts
    let port = 18000 + rand::random::<u16>() % 1000;
    let uri = format!("http://127.0.0.1:{}", port);

    // Create the SSE transport for the server
    let transport = SSEServerTransport::new(&uri)?;

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
        .with_name("SSE Test Server")
        .with_version("1.0.0")
        .with_tool(echo_tool);

    // Create the server
    let mut server = Server::new(server_config);

    // Register the echo tool handler
    server.register_tool_handler("echo", |params| async move {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::Protocol("Missing message parameter".to_string()))?;

        Ok(json!({
            "result": message
        }))
    })?;

    // Create a channel for signaling server shutdown
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Clone for background task
    let mut server_clone = server.clone();
    let transport_clone = transport.clone();

    // Start the server in a background task
    tokio::spawn(async move {
        tokio::select! {
            _ = server_clone.serve(transport_clone) => {
                println!("Server stopped");
            }
            _ = shutdown_rx.recv() => {
                println!("Server shutdown requested");
            }
        }
    });

    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok((uri, shutdown_tx))
}

/// Type alias for a pinned stream
type PinnedStream = Pin<Box<dyn Stream<Item = String> + Send>>;

/// Simple HTTP client for testing the server
struct TestClient {
    base_url: String,
    client: ReqwestClient,
}

impl TestClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: ReqwestClient::new(),
        }
    }

    /// Subscribe to SSE events
    async fn subscribe_to_events(&self) -> Result<PinnedStream, Box<dyn std::error::Error>> {
        let events_url = format!("{}/events", self.base_url);

        // Connect to SSE stream
        let response = self
            .client
            .get(&events_url)
            .header(header::ACCEPT, "text/event-stream")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to connect to SSE: HTTP {}", response.status()).into());
        }

        // Set up streaming
        let stream = response.bytes_stream();
        let byte_stream = StreamReader::new(
            stream.map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))),
        );
        let reader = BufReader::new(byte_stream);

        // Process the stream to extract SSE events
        let event_stream = futures::stream::unfold(
            (reader, String::new(), String::new()),
            |(mut reader, mut line, mut event_type)| async move {
                loop {
                    line.clear();
                    if let Ok(read) = reader.read_line(&mut line).await {
                        if read == 0 {
                            return None; // EOF
                        }

                        // Process event type lines
                        if line.starts_with("event:") {
                            event_type = line.trim_start_matches("event:").trim().to_string();
                            continue;
                        }

                        // Process data lines
                        if line.starts_with("data:") {
                            let data = line.trim_start_matches("data:").trim().to_string();
                            if event_type == "endpoint" {
                                println!("Received endpoint URL: {}", data);
                                // Continue reading, we want to return message events
                                continue;
                            } else {
                                // Return message events
                                return Some((data, (reader, line, event_type)));
                            }
                        }

                        // Skip empty lines or other SSE fields
                        continue;
                    } else {
                        return None; // Error
                    }
                }
            },
        );

        // Box and pin the stream
        Ok(Box::pin(event_stream))
    }

    /// Send a JSON-RPC message to the server
    async fn send_message(&self, message: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        let messages_url = format!("{}/messages", self.base_url);

        let response = self
            .client
            .post(&messages_url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(message)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to send message: HTTP {}", response.status()).into());
        }

        let json = response.json::<Value>().await?;
        Ok(json)
    }
}

#[tokio::test]
async fn test_sse_server() -> Result<(), Box<dyn std::error::Error>> {
    // Start the server
    let (server_url, shutdown_tx) = start_test_server().await?;
    println!("Test server started at {}", server_url);

    // Create a test client
    let client = TestClient::new(&server_url);

    // Create a stream of SSE events
    let mut event_stream = client.subscribe_to_events().await?;

    // Prepare and send initialization request
    let init_request = JSONRPCRequest::new(
        RequestId::Number(1),
        "initialize".to_string(),
        Some(json!({ "protocol_version": "0.1" })),
    );

    let init_result = client
        .send_message(&serde_json::to_value(JSONRPCMessage::Request(
            init_request,
        ))?)
        .await?;
    println!("Initialization response: {}", init_result);

    // Prepare and send tools/list request
    let tools_request = JSONRPCRequest::new(RequestId::Number(2), "tools/list".to_string(), None);

    let tools_result = client
        .send_message(&serde_json::to_value(JSONRPCMessage::Request(
            tools_request,
        ))?)
        .await?;
    println!("Tools list response: {}", tools_result);

    // Create an echo tool call request
    let echo_request = JSONRPCRequest::new(
        RequestId::Number(3),
        "tools/call".to_string(),
        Some(json!({
            "name": "echo",
            "arguments": {
                "message": "Hello from SSE test!"
            }
        })),
    );

    let echo_result = client
        .send_message(&serde_json::to_value(JSONRPCMessage::Request(
            echo_request,
        ))?)
        .await?;
    println!("Echo tool response: {}", echo_result);

    // Check that we can receive SSE events
    let mut received_events = Vec::new();

    // Wait for up to 5 events or timeout after 3 seconds
    let timeout_future = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(timeout_future);

    loop {
        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(data) => {
                        println!("Received SSE event: {}", data);
                        received_events.push(data);
                        if received_events.len() >= 5 {
                            break;
                        }
                    },
                    None => break,
                }
            }
            _ = &mut timeout_future => {
                println!("Timeout waiting for events");
                break;
            }
        }
    }

    // We should have received at least some events
    assert!(
        !received_events.is_empty(),
        "Should have received at least one SSE event"
    );

    // Shutdown the server
    shutdown_tx.send(()).await?;

    Ok(())
}
