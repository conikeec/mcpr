use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

/// Represents a session for SSE connections
pub struct Session {
    /// Unique identifier for the session
    pub id: String,

    /// Channel for sending messages to this specific session
    pub sender: mpsc::Sender<String>,
}

/// Manages multiple SSE sessions
pub struct SessionManager {
    /// Active SSE sessions, keyed by session ID
    active_sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,

    /// Broadcast channel for sending messages to all sessions
    broadcaster: Arc<broadcast::Sender<String>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(broadcaster: Arc<broadcast::Sender<String>>) -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            broadcaster,
        }
    }

    /// Get the active sessions storage
    pub fn sessions(&self) -> Arc<Mutex<HashMap<String, mpsc::Sender<String>>>> {
        self.active_sessions.clone()
    }

    /// Get the broadcaster
    pub fn broadcaster(&self) -> Arc<broadcast::Sender<String>> {
        self.broadcaster.clone()
    }

    /// Create a new session and register it
    pub async fn create_session(&self) -> Session {
        // Generate a unique session ID
        let session_id = Uuid::new_v4().to_string();

        // Create a channel for this session
        let (session_tx, _) = mpsc::channel::<String>(100);

        // Register the session
        {
            let mut sessions = self.active_sessions.lock().await;
            sessions.insert(session_id.clone(), session_tx.clone());
        }

        Session {
            id: session_id,
            sender: session_tx,
        }
    }

    /// Remove a session by ID
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        sessions.remove(session_id);
    }

    /// Send a message to a specific session
    pub async fn send_to_session(&self, session_id: &str, message: &str) -> Result<(), String> {
        let sessions = self.active_sessions.lock().await;
        if let Some(tx) = sessions.get(session_id) {
            tx.send(message.to_string())
                .await
                .map_err(|e| format!("Failed to send to session {}: {}", session_id, e))
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    /// Broadcast a message to all sessions
    pub fn broadcast(&self, message: &str) -> Result<(), String> {
        self.broadcaster
            .send(message.to_string())
            .map(|_| ())
            .map_err(|e| format!("Failed to broadcast message: {}", e))
    }

    /// Check if a session exists
    pub async fn session_exists(&self, session_id: &str) -> bool {
        let sessions = self.active_sessions.lock().await;
        sessions.contains_key(session_id)
    }

    /// Get the number of active sessions
    pub async fn session_count(&self) -> usize {
        let sessions = self.active_sessions.lock().await;
        sessions.len()
    }

    /// Handle an SSE connection
    pub async fn handle_sse_connection(
        &self,
        mut stream: TcpStream,
        host: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Generate a unique session ID
        let session_id = Uuid::new_v4().to_string();

        // Create a channel for this session
        let (session_tx, mut session_rx) = mpsc::channel::<String>(100);

        // Register the session
        {
            let mut sessions = self.active_sessions.lock().await;
            sessions.insert(session_id.clone(), session_tx);
        }

        // Send SSE headers
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;

        // Determine the message endpoint URL based on the request
        let scheme = "http"; // Default to HTTP
        let messages_endpoint = format!("{}://{}/messages?sessionId={}", scheme, host, session_id);

        // Send the endpoint event with session ID
        let endpoint_event = format!("event: endpoint\ndata: {}\n\n", messages_endpoint);
        stream.write_all(endpoint_event.as_bytes()).await?;
        stream.flush().await?;

        // Subscribe to broadcast channel
        let mut broadcast_rx = self.broadcaster.subscribe();

        // Send welcome message
        let welcome = serde_json::json!({
            "id": 0,
            "jsonrpc": "2.0",
            "method": "welcome",
            "params": {"message": "Connected to SSE stream", "session": session_id}
        });

        if let Ok(json) = serde_json::to_string(&welcome) {
            let sse_event = format!("event: message\ndata: {}\n\n", json);
            stream.write_all(sse_event.as_bytes()).await?;
            stream.flush().await?;
        }

        println!(
            "Client connected to SSE stream with session ID: {}",
            session_id
        );

        // Keep the connection alive until it's closed
        let mut connected = true;
        while connected {
            tokio::select! {
                // Check for session-specific messages
                msg = session_rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let sse_event = format!("event: message\ndata: {}\n\n", msg);
                            if let Err(_) = stream.write_all(sse_event.as_bytes()).await {
                                connected = false;
                                break;
                            }
                            if let Err(_) = stream.flush().await {
                                connected = false;
                                break;
                            }
                        },
                        None => {
                            // Channel closed
                            connected = false;
                            break;
                        }
                    }
                },
                // Check for broadcast messages
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(msg) => {
                            // Only forward broadcast messages that don't have a session
                            // or match this session's ID
                            if let Ok(value) = serde_json::from_str::<Value>(&msg) {
                                if value.get("session").is_none() ||
                                   value.get("session") == Some(&Value::String(session_id.clone())) {
                                    let sse_event = format!("event: message\ndata: {}\n\n", msg);
                                    if let Err(_) = stream.write_all(sse_event.as_bytes()).await {
                                        connected = false;
                                        break;
                                    }
                                    if let Err(_) = stream.flush().await {
                                        connected = false;
                                        break;
                                    }
                                }
                            } else {
                                // Non-JSON messages are broadcast to everyone
                                let sse_event = format!("event: message\ndata: {}\n\n", msg);
                                if let Err(_) = stream.write_all(sse_event.as_bytes()).await {
                                    connected = false;
                                    break;
                                }
                                if let Err(_) = stream.flush().await {
                                    connected = false;
                                    break;
                                }
                            }
                        },
                        Err(_) => {
                            // Channel closed
                            connected = false;
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup when the connection is closed
        self.remove_session(&session_id).await;

        Ok(())
    }

    /// Handle a POST request to send a message
    pub async fn handle_post_request(
        &self,
        mut stream: TcpStream,
        message_store: Arc<Mutex<Vec<String>>>,
        message_tx: Arc<mpsc::Sender<String>>,
        session_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse HTTP request to extract headers and body
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).await?;

        if n == 0 {
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buffer[..n]);

        // Find the separator between headers and body (empty line)
        let mut body = String::new();
        if let Some(headers_end) = request.find("\r\n\r\n") {
            // Body starts after the empty line that separates headers from body
            let body_start = headers_end + 4; // Skip \r\n\r\n
            if body_start < request.len() {
                body = request[body_start..].to_string();
            }
        }

        // If body is empty, check for Content-Length and read more data if needed
        if body.is_empty() || body.trim().is_empty() {
            // Find Content-Length header
            let mut content_length = 0;
            for line in request.lines() {
                if line.to_lowercase().starts_with("content-length:") {
                    if let Some(len_str) = line.split(':').nth(1) {
                        if let Ok(len) = len_str.trim().parse::<usize>() {
                            content_length = len;
                        }
                    }
                }
            }

            if content_length > 0 {
                // Read more data if needed
                let mut body_buffer = vec![0; content_length];
                stream.read_exact(&mut body_buffer).await?;
                body = String::from_utf8_lossy(&body_buffer).to_string();
            }
        }

        // Process the message body
        if body.trim().is_empty() {
            // Return bad request if body is empty
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nEmpty body\n";
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        }

        // Try to parse the body as JSON
        let is_request = if let Ok(value) = serde_json::from_str::<Value>(&body) {
            // Check if it's a request (has method field)
            value.get("method").is_some()
        } else {
            // Return bad request if body is not valid JSON
            let response =
                "HTTP/1.1 400 Bad Request\r\nContent-Length: 16\r\n\r\nInvalid JSON body\n";
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        };

        if is_request {
            // It's a request, store it and send to the message channel for processing
            let mut messages = message_store.lock().await;
            messages.push(body.clone());

            // Send to the message channel for receive() method
            let _ = message_tx.send(body.clone()).await;

            // Don't send the request back to clients - the server will generate responses
        } else {
            // It's not a request (probably a response); just forward it to the right client
            if let Some(session) = &session_id {
                let _ = self.send_to_session(session, &body).await;
            }
        }

        // Send HTTP response
        let response = serde_json::json!({
            "success": true,
            "message": "Message received and processed"
        });
        let json = serde_json::to_string(&response)?;
        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            json.len(),
            json
        );
        stream.write_all(http_response.as_bytes()).await?;

        Ok(())
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            active_sessions: self.active_sessions.clone(),
            broadcaster: self.broadcaster.clone(),
        }
    }
}
