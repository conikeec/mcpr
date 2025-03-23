// cspell:ignore oneshot
#![cfg(test)]
use crate::transport::sse::{SSEClientTransport, SSEServerTransport};
use crate::transport::Transport;
use bytes::Bytes;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use warp::{Filter, Reply};

// Test message structure matching the protocol
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct TestMessage {
    id: u32,
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
}

// Helper function to create a mock SSE server using warp
async fn create_mock_sse_server(
    post_addr: Option<SocketAddr>,
) -> (SocketAddr, oneshot::Sender<()>) {
    // Create a shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Build a Vec of test messages to send
    let test_messages = Arc::new(vec![
        TestMessage {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: "test1".to_string(),
            params: serde_json::json!({}),
        },
        TestMessage {
            id: 2,
            jsonrpc: "2.0".to_string(),
            method: "test2".to_string(),
            params: serde_json::json!({
                "key": "value"
            }),
        },
    ]);

    // Create a route that sends SSE events
    let test_msgs = test_messages.clone();
    let post_address = post_addr.clone();

    // Use a simpler approach - directly create the SSE endpoint with a response
    let sse_route = warp::path("events").and(warp::get()).map(move || {
        let messages = test_msgs.clone();
        let post_addr_opt = post_address.clone();

        // Create a buffer for the response
        let mut buffer = String::new();

        // Add endpoint event
        let endpoint_url = if let Some(post_address) = post_addr_opt {
            format!("http://{}/messages", post_address)
        } else {
            "http://localhost:8000/messages".to_string()
        };
        buffer.push_str(&format!("event: endpoint\ndata: {}\n\n", endpoint_url));

        // Add test messages
        for msg in messages.iter() {
            if let Ok(json) = serde_json::to_string(msg) {
                buffer.push_str(&format!("event: message\ndata: {}\n\n", json));
            }
        }

        // Set headers for SSE
        warp::reply::with_header(buffer, "content-type", "text/event-stream")
    });

    // Run the server
    let (addr, server) =
        warp::serve(sse_route).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
            shutdown_rx.await.ok();
        });

    // Spawn the server
    tokio::spawn(server);

    // Return the address and shutdown handle
    (addr, shutdown_tx)
}

// Helper function to create a mock HTTP POST endpoint using warp
async fn create_mock_post_endpoint() -> (SocketAddr, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>) {
    // Create a shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Create a shared collection to store received messages
    let received_messages = Arc::new(Mutex::new(Vec::<String>::new()));
    let received_messages_clone = received_messages.clone();

    // Create the POST route
    let post_route = warp::path("messages")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |message: serde_json::Value| {
            // Store the received message
            let message_str = message.to_string();
            println!("Received message: {}", message_str);

            // Clone for async task
            let messages = received_messages_clone.clone();

            tokio::spawn(async move {
                let mut messages_guard = messages.lock().await;
                messages_guard.push(message_str);
            });

            // Send a success response
            warp::reply::json(&serde_json::json!({
                "status": "success",
                "message": "Message received"
            }))
        });

    // Run the server
    let (addr, server) =
        warp::serve(post_route).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
            shutdown_rx.await.ok();
        });

    // Spawn the server
    tokio::spawn(server);

    // Return the address, shutdown handle, and received messages collection
    (addr, shutdown_tx, received_messages)
}

// Test for sending SSE messages
#[tokio::test]
async fn test_sse_transport_send() {
    // First create the POST endpoint to get its address
    let (post_addr, post_shutdown_tx, received_messages) = create_mock_post_endpoint().await;
    println!("POST endpoint listening on: {}", post_addr);

    // Now create the SSE server with the POST endpoint address
    let (server_addr, shutdown_tx) = create_mock_sse_server(Some(post_addr)).await;

    let sse_url = format!("http://{}/events", server_addr);
    println!("SSE server URL: {}", sse_url);
    println!("POST endpoint address: {}", post_addr);

    // Create the SSE client transport with auth token
    let mut transport = SSEClientTransport::new(&sse_url)
        .unwrap()
        .with_auth_token("test-token");

    // Start the transport
    println!("Starting transport");
    transport.start().await.unwrap();

    // Wait for the endpoint event to be processed
    println!("Waiting for endpoint event to be processed...");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create a test message to send
    let message = TestMessage {
        id: 3,
        jsonrpc: "2.0".to_string(),
        method: "test3".to_string(),
        params: serde_json::json!({
            "key": "value",
            "nested": {
                "nestedKey": "nestedValue"
            }
        }),
    };

    // Send the message
    println!("Sending message");
    transport.send(&message).await.unwrap();

    // Give the server some time to process the message
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify that the message was received by the POST endpoint
    let received = received_messages.lock().await;
    assert!(!received.is_empty(), "No messages received");

    if !received.is_empty() {
        let received_message: TestMessage = serde_json::from_str(&received[0]).unwrap();
        assert_eq!(received_message.id, 3);
        assert_eq!(received_message.method, "test3");
    }

    // Clean up
    println!("Cleaning up");
    transport.close().await.unwrap();
    let _ = shutdown_tx.send(());
    let _ = post_shutdown_tx.send(());
}

// Test that the auth token is properly set and used
#[tokio::test]
async fn test_sse_transport_with_auth() {
    let transport = SSEClientTransport::new("http://localhost:8000/events")
        .unwrap()
        .with_auth_token("test-token");

    assert!(transport.has_auth_token());
    assert_eq!(transport.get_auth_token(), Some("test-token"));
}

// Test that reconnection parameters are properly set
#[tokio::test]
async fn test_sse_transport_reconnect_params() {
    let transport = SSEClientTransport::new("http://localhost:8000/events")
        .unwrap()
        .with_reconnect_params(10, 3);

    assert_eq!(transport.get_reconnect_interval(), Duration::from_secs(10));
    assert_eq!(transport.get_max_reconnect_attempts(), 3);
}

// Test that cloning the transport works as expected
#[tokio::test]
async fn test_sse_transport_clone() {
    let transport = SSEClientTransport::new("http://localhost:8000/events")
        .unwrap()
        .with_auth_token("test-token")
        .with_reconnect_params(10, 3);

    let cloned = transport.clone();

    assert_eq!(cloned.get_auth_token(), Some("test-token"));
    assert_eq!(cloned.get_reconnect_interval(), Duration::from_secs(10));
    assert_eq!(cloned.get_max_reconnect_attempts(), 3);
}

// Test for the SSE server transport
#[tokio::test]
async fn test_sse_server_transport() {
    let server_url = "http://127.0.0.1:0"; // Let the system assign a port
    let mut server = SSEServerTransport::new(server_url).unwrap();

    // Start the server
    server.start().await.unwrap();

    // Give it some time to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clean up
    server.close().await.unwrap();
}
