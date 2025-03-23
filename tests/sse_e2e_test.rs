use futures::{Stream, StreamExt};
use mcpr::{
    client::Client as MCPClient,
    error::MCPError,
    schema::{
        client::CallToolParams,
        common::{Tool, ToolInputSchema},
        json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    },
    server::{Server, ServerConfig},
    transport::{
        sse::{SSEClientTransport, SSEServerTransport},
        Transport,
    },
};
use reqwest::{self, header};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
    time::timeout,
};
use tokio_util::io::StreamReader;

/// Run a basic SSE server for testing
async fn run_test_server() -> Result<(String, mpsc::Sender<()>), MCPError> {
    // Use a random port to avoid conflicts
    let port = 18000 + rand::random::<u16>() % 1000;
    let uri = format!("http://127.0.0.1:{}", port);
    let transport = SSEServerTransport::new(&uri)?;

    // Configure a simple echo tool
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

    // Create server config with the echo tool
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

    // Create shutdown channel
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Clone the transport and server for the task
    let mut server_clone = server.clone();
    let transport_clone = transport.clone();

    // Start the server in a background task
    tokio::spawn(async move {
        // Start the server
        let server_fut = server_clone.serve(transport_clone);

        // Wait for either server completion or shutdown signal
        tokio::select! {
            _ = server_fut => {},
            _ = shutdown_rx.recv() => {},
        }
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok((uri, shutdown_tx))
}

/// Helper to collect SSE messages as they arrive
async fn collect_sse_messages(
    uri: &str,
    limit: usize,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let sse_url = format!("{}/events", uri);

    // Connect to the SSE endpoint
    let res = client
        .get(&sse_url)
        .header(header::ACCEPT, "text/event-stream")
        .send()
        .await?;

    // Ensure successful connection
    if !res.status().is_success() {
        return Err(format!("Failed to connect: HTTP {}", res.status()).into());
    }

    // Set up a stream reader
    let stream = res.bytes_stream();
    let byte_stream = StreamReader::new(
        stream.map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))),
    );
    let mut reader = BufReader::new(byte_stream);

    // Collect messages
    let mut messages = Vec::new();
    let mut line = String::new();

    loop {
        // Break if we've collected enough messages
        if messages.len() >= limit {
            break;
        }

        // Read next line with timeout
        match timeout(Duration::from_secs(5), reader.read_line(&mut line)).await {
            Ok(Ok(bytes)) if bytes > 0 => {
                // Process SSE line
                if line.starts_with("data:") {
                    let data = line.trim_start_matches("data:").trim();
                    messages.push(data.to_string());
                }
                line.clear();
            }
            _ => break,
        }
    }

    Ok(messages)
}

/// Send a message to the server
async fn send_message(uri: &str, message: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let send_url = format!("{}/messages", uri);

    // POST the message
    let res = client
        .post(&send_url)
        .header(header::CONTENT_TYPE, "application/json")
        .json(message)
        .send()
        .await?;

    // Ensure successful response
    if !res.status().is_success() {
        return Err(format!("Failed to send message: HTTP {}", res.status()).into());
    }

    // Parse the response
    let response = res.json::<Value>().await?;
    Ok(response)
}

#[tokio::test]
async fn test_sse_e2e() -> Result<(), Box<dyn std::error::Error>> {
    // Start test server
    let (uri, shutdown_tx) = run_test_server().await?;
    println!("Test server started at {}", uri);

    // Start collecting messages
    let messages_fut = collect_sse_messages(&uri, 3);

    // Give time for SSE connection to establish
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Prepare and send initialization request
    let init_request = JSONRPCRequest::new(
        RequestId::Number(1),
        "initialize".to_string(),
        Some(json!({
            "protocol_version": "0.1"
        })),
    );

    let init_result = send_message(
        &uri,
        &serde_json::to_value(JSONRPCMessage::Request(init_request))?,
    )
    .await?;
    println!("Initialization response: {}", init_result);

    // Prepare and send tools/list request
    let tools_request = JSONRPCRequest::new(RequestId::Number(2), "tools/list".to_string(), None);

    let tools_result = send_message(
        &uri,
        &serde_json::to_value(JSONRPCMessage::Request(tools_request))?,
    )
    .await?;
    println!("Tools list response: {}", tools_result);

    // Verify that the tools list contains our echo tool
    let tools = tools_result
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    assert!(tools, "Expected successful tools/list response");

    // Prepare and send an echo tool call
    let call_params = CallToolParams {
        name: "echo".to_string(),
        arguments: Some(HashMap::from([(
            "message".to_string(),
            json!("Hello from E2E test!"),
        )])),
    };

    let call_request = JSONRPCRequest::new(
        RequestId::Number(3),
        "tools/call".to_string(),
        Some(serde_json::to_value(call_params)?),
    );

    let call_result = send_message(
        &uri,
        &serde_json::to_value(JSONRPCMessage::Request(call_request))?,
    )
    .await?;
    println!("Tool call response: {}", call_result);

    // Wait to collect all messages from SSE stream
    let collected_messages = messages_fut.await?;

    // Verify we received expected SSE messages
    println!("Collected SSE messages: {:?}", collected_messages);
    assert!(
        !collected_messages.is_empty(),
        "Expected to receive SSE messages"
    );

    // Shutdown the server
    shutdown_tx.send(()).await?;

    Ok(())
}
