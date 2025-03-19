use crate::error::MCPError;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use log::{debug, error, info, warn};
use reqwest::blocking::{Client, Response};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tiny_http::{Method, Response as HttpResponse, Server};

/// Client connection information
struct ClientConnection {
    #[allow(dead_code)]
    id: String,
    last_poll: Instant,
}

/// Server-Sent Events (SSE) transport
pub struct SSETransport {
    uri: String,
    is_connected: bool,
    is_server: bool,
    on_close: Option<CloseCallback>,
    on_error: Option<ErrorCallback>,
    on_message: Option<MessageCallback>,
    // HTTP client for making requests
    #[allow(dead_code)]
    client: Client,
    // Queue for incoming messages
    message_queue: Arc<Mutex<VecDeque<String>>>,
    // Thread for polling SSE events
    receiver_thread: Option<thread::JoinHandle<()>>,
    // For server mode: active client connections
    active_clients: Arc<Mutex<HashMap<String, ClientConnection>>>,
    // For server mode: client message queues
    client_messages: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
    // For client mode: client ID
    client_id: Arc<Mutex<Option<String>>>,
    // Server instance
    server: Option<Arc<Server>>,
}

impl SSETransport {
    /// Create a new SSE transport
    pub fn new(uri: &str) -> Self {
        info!("Creating new SSE transport with URI: {}", uri);
        Self {
            uri: uri.to_string(),
            is_connected: false,
            is_server: false,
            on_close: None,
            on_error: None,
            on_message: None,
            client: Client::new(),
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            receiver_thread: None,
            active_clients: Arc::new(Mutex::new(HashMap::new())),
            client_messages: Arc::new(Mutex::new(HashMap::new())),
            client_id: Arc::new(Mutex::new(None)),
            server: None,
        }
    }

    /// Create a new SSE transport in server mode
    pub fn new_server(uri: &str) -> Self {
        info!("Creating new SSE server transport with URI: {}", uri);
        let mut transport = Self::new(uri);
        transport.is_server = true;
        transport
    }

    /// Handle an error by calling the error callback if set
    #[allow(dead_code)]
    fn handle_error(&self, error: &MCPError) {
        error!("SSE transport error: {}", error);
        if let Some(callback) = &self.on_error {
            callback(error);
        }
    }

    /// Process SSE events from a response
    #[allow(dead_code)]
    fn process_sse_events(&self, response: Response) {
        let message_queue = Arc::clone(&self.message_queue);

        // Read the response body line by line
        let reader = response.text().unwrap_or_default();

        // Process SSE events
        for line in reader.lines() {
            if line.starts_with("data: ") {
                let data = line.trim_start_matches("data: ");
                debug!("Received SSE event: {}", data);

                // Add the message to the queue
                if let Ok(mut queue) = message_queue.lock() {
                    queue.push_back(data.to_string());
                }

                // Call the message callback if set
                if let Some(callback) = &self.on_message {
                    callback(data);
                }
            }
        }
    }
}

impl Transport for SSETransport {
    fn start(&mut self) -> Result<(), MCPError> {
        if self.is_connected {
            debug!("SSE transport already connected");
            return Err(MCPError::AlreadyConnected);
        }

        info!("Starting SSE transport with URI: {}", self.uri);

        // Create a message queue for the receiver thread
        let message_queue = Arc::clone(&self.message_queue);

        if self.is_server {
            // Parse the URI to get the host and port
            let uri = self.uri.clone();
            let uri_parts: Vec<&str> = uri.split("://").collect();
            if uri_parts.len() != 2 {
                return Err(MCPError::Transport(format!("Invalid URI: {}", uri)));
            }

            let addr_parts: Vec<&str> = uri_parts[1].split(':').collect();
            if addr_parts.len() != 2 {
                return Err(MCPError::Transport(format!("Invalid URI: {}", uri)));
            }

            let host = addr_parts[0];
            let port: u16 = match addr_parts[1].parse() {
                Ok(p) => p,
                Err(_) => return Err(MCPError::Transport(format!("Invalid port in URI: {}", uri))),
            };

            let addr = format!("{}:{}", host, port);
            info!("Starting SSE server on {}", addr);

            // Create the HTTP server
            let server = match Server::http(&addr) {
                Ok(s) => s,
                Err(e) => {
                    return Err(MCPError::Transport(format!(
                        "Failed to start HTTP server: {}",
                        e
                    )))
                }
            };

            let server_arc = Arc::new(server);
            self.server = Some(Arc::clone(&server_arc));

            // Start a thread to handle incoming requests
            let active_clients = Arc::clone(&self.active_clients);
            let client_messages = Arc::clone(&self.client_messages);
            let server_thread = thread::spawn(move || {
                let server = server_arc;

                for mut request in server.incoming_requests() {
                    let method = request.method();
                    let url = request.url().to_string();

                    debug!("Server received {} request for {}", method, url);

                    match (method, url.as_str()) {
                        (Method::Post, "/") => {
                            // Handle POST request (client sending a message to server)
                            let mut content = String::new();
                            if let Err(e) = request.as_reader().read_to_string(&mut content) {
                                error!("Error reading request body: {}", e);
                                let _ = request.respond(
                                    HttpResponse::from_string("Error reading request")
                                        .with_status_code(400),
                                );
                                continue;
                            }

                            debug!("Server received POST request body: {}", content);

                            // Add the message to the server's message queue for processing
                            if let Ok(mut queue) = message_queue.lock() {
                                queue.push_back(content);
                                debug!("Added message to server queue for processing");
                            }

                            // Send a success response
                            let _ = request
                                .respond(HttpResponse::from_string("OK").with_status_code(200));
                        }
                        (Method::Get, path) if path.starts_with("/poll") => {
                            // Handle polling request from client
                            debug!("Server received polling request: {}", path);

                            // Extract client ID from query parameters
                            let client_id = path.split('?').nth(1).and_then(|query| {
                                query.split('&').find_map(|pair| {
                                    let mut parts = pair.split('=');
                                    if let Some(key) = parts.next() {
                                        if key == "client_id" {
                                            parts.next().map(|value| value.to_string())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                            });

                            if let Some(client_id) = client_id {
                                // Check if there are any messages in the client-specific queue
                                let message = if let Ok(mut client_msgs) = client_messages.lock() {
                                    client_msgs
                                        .entry(client_id.clone())
                                        .or_insert_with(VecDeque::new)
                                        .pop_front()
                                } else {
                                    None
                                };

                                // Send the message or a no-message response
                                if let Some(msg) = message {
                                    debug!(
                                        "Server sending message to client {}: {}",
                                        client_id, msg
                                    );
                                    let response = HttpResponse::from_string(msg)
                                        .with_status_code(200)
                                        .with_header(tiny_http::Header {
                                            field: "Content-Type".parse().unwrap(),
                                            value: "application/json".parse().unwrap(),
                                        });

                                    if let Err(e) = request.respond(response) {
                                        error!("Failed to send response to client: {}", e);
                                    } else {
                                        debug!("Server successfully sent response to client");
                                    }
                                } else {
                                    // No messages available
                                    debug!(
                                        "Server sending no_messages response to client {}",
                                        client_id
                                    );
                                    let response = HttpResponse::from_string("no_messages")
                                        .with_status_code(200);

                                    if let Err(e) = request.respond(response) {
                                        error!("Failed to send no_messages response: {}", e);
                                    }
                                }

                                // Update the client's last poll time
                                if let Ok(mut clients) = active_clients.lock() {
                                    if let Some(client) = clients.get_mut(&client_id) {
                                        client.last_poll = Instant::now();
                                    }
                                }
                            } else {
                                // No client ID provided
                                debug!("Client poll request missing client_id parameter");
                                let response =
                                    HttpResponse::from_string("Missing client_id parameter")
                                        .with_status_code(400);
                                let _ = request.respond(response);
                            }
                        }
                        (Method::Get, "/register") => {
                            // Handle client registration
                            debug!("Server received client registration request");

                            // Track the client connection
                            let client_id = format!(
                                "client-{}",
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                            );

                            if let Ok(mut clients) = active_clients.lock() {
                                clients.insert(
                                    client_id.clone(),
                                    ClientConnection {
                                        id: client_id.clone(),
                                        last_poll: Instant::now(),
                                    },
                                );
                                debug!("Client registered: {}", client_id);
                                debug!("Total connected clients: {}", clients.len());
                            }

                            // Initialize the client's message queue
                            if let Ok(mut client_msgs) = client_messages.lock() {
                                client_msgs
                                    .entry(client_id.clone())
                                    .or_insert_with(VecDeque::new);
                                debug!("Initialized message queue for client {}", client_id);
                            }

                            // Send a success response
                            let response = HttpResponse::from_string(format!(
                                "{{\"client_id\":\"{}\"}}",
                                client_id
                            ))
                            .with_status_code(200)
                            .with_header(tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: "application/json".parse().unwrap(),
                            });

                            if let Err(e) = request.respond(response) {
                                error!("Failed to send registration response: {}", e);
                            } else {
                                debug!("Server successfully registered client");
                            }
                        }
                        _ => {
                            // Unsupported method or path
                            error!("Unsupported request: {} {}", method, url);
                            let _ = request.respond(
                                HttpResponse::from_string("Method or path not allowed")
                                    .with_status_code(405),
                            );
                        }
                    }
                }
            });

            self.receiver_thread = Some(server_thread);
        } else {
            // Client mode - we'll use a simpler approach with polling
            let uri = self.uri.clone();
            let client = Client::new();
            let is_connected = Arc::new(Mutex::new(true));
            let is_connected_clone = Arc::clone(&is_connected);
            let client_id = Arc::clone(&self.client_id);

            // Register with the server
            debug!("Client registering with server at {}/register", uri);
            match client.get(format!("{}/register", uri)).send() {
                Ok(response) => {
                    if response.status().is_success() {
                        // Parse the client ID from the response
                        match response.text() {
                            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    if let Some(id) =
                                        json.get("client_id").and_then(|id| id.as_str())
                                    {
                                        debug!("Client registration successful with ID: {}", id);
                                        if let Ok(mut client_id_guard) = client_id.lock() {
                                            *client_id_guard = Some(id.to_string());
                                        }
                                    } else {
                                        warn!(
                                            "Client registration response missing client_id field"
                                        );
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse client registration response: {}", e);
                                }
                            },
                            Err(e) => {
                                warn!("Failed to read client registration response: {}", e);
                            }
                        }
                    } else {
                        warn!("Client registration failed: HTTP {}", response.status());
                    }
                }
                Err(e) => {
                    warn!("Client registration failed: {}", e);
                }
            }

            // Start a thread to poll for messages
            let client_id_clone = Arc::clone(&client_id);
            let receiver_thread = thread::spawn(move || {
                while let Ok(connected) = is_connected_clone.lock() {
                    if !*connected {
                        debug!("Polling thread detected transport closure, exiting");
                        break;
                    }

                    // Get the client ID
                    let client_id_str = if let Ok(id_guard) = client_id_clone.lock() {
                        id_guard.clone()
                    } else {
                        None
                    };

                    // Send a GET request to poll for messages
                    let poll_uri = if let Some(id) = &client_id_str {
                        format!("{}/poll?client_id={}", uri, id)
                    } else {
                        format!("{}/poll", uri)
                    };
                    debug!("Client polling for messages at {}", poll_uri);

                    match client.get(&poll_uri).send() {
                        Ok(response) => {
                            if response.status().is_success() {
                                match response.text() {
                                    Ok(text) => {
                                        if !text.is_empty() && text != "no_messages" {
                                            debug!("Client received message from poll: {}", text);

                                            // Try to parse as JSON to validate
                                            match serde_json::from_str::<serde_json::Value>(&text) {
                                                Ok(_) => {
                                                    // Add the message to the queue
                                                    if let Ok(mut queue) = message_queue.lock() {
                                                        queue.push_back(text);
                                                        debug!("Client added message to queue for processing");
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("Client received invalid JSON from server: {} - {}", e, text);
                                                }
                                            }
                                        } else {
                                            debug!("Client: No new messages available");
                                        }
                                    }
                                    Err(e) => {
                                        error!("Client failed to read response text: {}", e);
                                    }
                                }
                            } else {
                                error!("Client poll request failed: HTTP {}", response.status());
                            }
                        }
                        Err(e) => {
                            error!("Client failed to poll for messages: {}", e);
                            // Add a small delay before retrying to avoid hammering the server
                            thread::sleep(Duration::from_millis(1000));
                        }
                    }

                    // Wait before polling again
                    thread::sleep(Duration::from_millis(500));
                }
                debug!("Client polling thread exited");
            });

            self.receiver_thread = Some(receiver_thread);
        }

        self.is_connected = true;
        info!("SSE transport started successfully");
        Ok(())
    }



    fn close(&mut self) -> Result<(), MCPError> {
        if !self.is_connected {
            debug!("SSE transport already closed");
            return Ok(());
        }

        info!("Closing SSE transport for URI: {}", self.uri);

        // Set the connection flag
        self.is_connected = false;

        // If we're a server, wait a short time to allow clients to receive final responses
        if self.is_server {
            debug!("Server waiting for clients to receive final responses");
            // Wait a short time to allow clients to receive final responses
            thread::sleep(Duration::from_millis(1000));
        }

        // Signal the polling thread to stop if it exists
        if let Some(_thread) = self.receiver_thread.take() {
            // We can't join the thread here because it might be blocked on I/O
            // Just let it detect the is_connected flag and exit naturally
            debug!("Signaled receiver thread to stop");
        }

        // Call the close callback if set
        if let Some(callback) = &self.on_close {
            callback();
        }

        info!("SSE transport closed successfully");
        Ok(())
    }

    fn set_on_close(&mut self, callback: Option<CloseCallback>) {
        debug!("Setting on_close callback for SSE transport");
        self.on_close = callback;
    }

    fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
        debug!("Setting on_error callback for SSE transport");
        self.on_error = callback;
    }

    fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        self.on_close = Some(callback);
    }

    fn on_error(&mut self, callback: Box<dyn Fn(&MCPError) + Send + Sync>) {
        self.on_error = Some(callback);
    }

    fn on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
        self.on_message = Some(callback);
    }

    fn send_json(&mut self, json_string: &str) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            return Err(error);
        }

        // Implementation depends on whether we're in client or server mode
        if self.is_server {
            // In server mode, find the client to send to
            if let Ok(client_id) = self.client_id.lock() {
                if let Some(id) = client_id.as_ref() {
                    // Add message to client's queue
                    if let Ok(mut client_msgs) = self.client_messages.lock() {
                        let queue = client_msgs.entry(id.clone()).or_insert_with(VecDeque::new);
                        queue.push_back(json_string.to_string());
                        return Ok(());
                    }
                }
            }
            Err(MCPError::Transport("No client to send to".to_string()))
        } else {
            // In client mode, send message to server via HTTP POST
            let client = Client::new();
            let response = match client.post(&self.uri).body(json_string.to_string()).send() {
                Ok(response) => response,
                Err(e) => {
                    let error =
                        MCPError::Transport(format!("Failed to send message to server: {}", e));
                    self.handle_error(&error);
                    return Err(error);
                }
            };

            if !response.status().is_success() {
                let error =
                    MCPError::Transport(format!("Server returned error: {}", response.status()));
                self.handle_error(&error);
                return Err(error);
            }

            Ok(())
        }
    }

    fn receive_json(&mut self) -> Result<String, MCPError> {
        if !self.is_connected {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            return Err(error);
        }

        // Try to get a message from the queue
        if let Ok(mut queue) = self.message_queue.lock() {
            if let Some(message) = queue.pop_front() {
                return Ok(message);
            }
        }

        Err(MCPError::Transport("No messages available".to_string()))
    }
}
