use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    id: u32,
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
}

// Store for received messages
type MessageStore = Arc<Mutex<Vec<Message>>>;

// Parse HTTP request to get path and headers
async fn parse_http_request(stream: &mut TcpStream) -> Option<(String, Vec<String>)> {
    let mut buffer = [0; 4096];
    let n = stream.read(&mut buffer).await.ok()?;

    if n == 0 {
        return None;
    }

    let request = String::from_utf8_lossy(&buffer[..n]);
    let lines: Vec<&str> = request.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // Parse the request line
    let request_line = lines[0];
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    // Extract path
    let path = parts[1].to_string();

    // Extract headers (skip request line)
    let headers = lines.iter().skip(1).map(|line| line.to_string()).collect();

    Some((path, headers))
}

// Handle SSE connection
async fn handle_sse_connection(
    mut stream: TcpStream,
    tx: broadcast::Sender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse HTTP request
    let (path, _headers) = parse_http_request(&mut stream)
        .await
        .ok_or("Failed to parse HTTP request")?;

    if path != "/events" {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 14\r\n\r\nPath not found";
        stream.write_all(response.as_bytes()).await?;
        println!("Rejected connection to invalid path: {}", path);
        return Ok(());
    }

    // Send SSE headers
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
    stream.write_all(response.as_bytes()).await?;

    // Subscribe to broadcast channel
    let mut rx = tx.subscribe();

    // Send welcome message
    let welcome = Message {
        id: 0,
        jsonrpc: "2.0".to_string(),
        method: "welcome".to_string(),
        params: serde_json::json!({"message": "Connected to SSE stream"}),
    };

    if let Ok(json) = serde_json::to_string(&welcome) {
        let sse_event = format!("data: {}\n\n", json);
        stream.write_all(sse_event.as_bytes()).await?;
        stream.flush().await?;
    }

    println!("Client connected to SSE stream");

    // Keep sending events
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if let Ok(json) = serde_json::to_string(&msg) {
                    let sse_event = format!("data: {}\n\n", json);

                    if let Err(_) = stream.write_all(sse_event.as_bytes()).await {
                        break; // Client disconnected
                    }

                    if let Err(_) = stream.flush().await {
                        break; // Client disconnected
                    }
                }
            }
            Err(_) => break, // Channel closed
        }
    }

    println!("Client disconnected from SSE stream");
    Ok(())
}

// Handle HTTP POST request
async fn handle_post_request(
    mut stream: TcpStream,
    tx: broadcast::Sender<Message>,
    message_store: MessageStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse HTTP request
    let (path, headers) = parse_http_request(&mut stream)
        .await
        .ok_or("Failed to parse HTTP request")?;

    if path != "/messages" {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 14\r\n\r\nPath not found";
        stream.write_all(response.as_bytes()).await?;
        println!("Rejected POST to invalid path: {}", path);
        return Ok(());
    }

    // Find Content-Length header
    let mut content_length = 0;
    for header in headers {
        if header.to_lowercase().starts_with("content-length:") {
            if let Some(len_str) = header.split(':').nth(1) {
                if let Ok(len) = len_str.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }
    }

    // Read the request body
    let mut body = vec![0; content_length];
    stream.read_exact(&mut body).await?;

    // Parse the message
    match serde_json::from_slice::<Message>(&body) {
        Ok(message) => {
            println!("Received message: {:?}", message);

            // Store the message
            {
                let mut store = message_store.lock().unwrap();
                store.push(message.clone());
            }

            // Create a response
            let response = Message {
                id: message.id,
                jsonrpc: "2.0".to_string(),
                method: "response".to_string(),
                params: serde_json::json!({
                    "success": true,
                    "received": message.method,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            };

            // Broadcast the response
            tx.send(response.clone())?;

            // Send HTTP response
            let json = serde_json::to_string(&response)?;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes()).await?;
        }
        Err(e) => {
            println!("Error parsing message: {}", e);

            // Send error response
            let error_msg = format!("Invalid message format: {}", e);
            let response = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}", error_msg.len(), error_msg);
            stream.write_all(response.as_bytes()).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a broadcast channel for messages
    let (tx, _) = broadcast::channel::<Message>(100);

    // Create a message store
    let message_store = Arc::new(Mutex::new(Vec::new()));

    // Set up the server address
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));

    // Create a TCP listener
    let listener = TcpListener::bind(addr).await?;

    println!("SSE server listening on http://{}", addr);
    println!("Endpoints:");
    println!("  - GET  http://{}/events   (SSE events stream)", addr);
    println!("  - POST http://{}/messages (Message endpoint)", addr);
    println!("\nTo test, run in another terminal: cargo run --example sse_client");

    // Spawn a task to periodically send heartbeat messages
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut counter = 0;
        loop {
            sleep(Duration::from_secs(10)).await;
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

            let _ = tx_clone.send(heartbeat);
        }
    });

    // Accept connections
    loop {
        let (mut stream, _) = listener.accept().await?;
        let tx_clone = tx.clone();
        let store_clone = message_store.clone();

        // Spawn a new task for each connection
        tokio::spawn(async move {
            // First read a bit to determine if it's GET or POST
            let mut peek_buffer = [0; 128];
            let n = match stream.peek(&mut peek_buffer).await {
                Ok(n) => n,
                Err(_) => return,
            };

            let peek_str = String::from_utf8_lossy(&peek_buffer[..n]);

            // Handle based on method
            if peek_str.starts_with("GET") {
                let _ = handle_sse_connection(stream, tx_clone).await;
            } else if peek_str.starts_with("POST") {
                let _ = handle_post_request(stream, tx_clone, store_clone).await;
            } else {
                // Unknown method
                let response = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 18\r\n\r\nMethod Not Allowed";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
    }
}
