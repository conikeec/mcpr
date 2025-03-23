// cspell:ignore reqwest
use crate::error::MCPError;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::StreamExt;
use log::{debug, warn};
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use url::Url;

/// Server-Sent Events (SSE) Client Transport
pub struct SSEClientTransport {
    /// The URL for SSE events
    url: Url,

    /// The URL for sending requests (dynamically received from server)
    send_url: Arc<Mutex<Option<Url>>>,

    /// HTTP client for requests
    http_client: Client,

    /// Authentication token for requests
    auth_token: Option<String>,

    /// Reconnection interval in seconds
    reconnect_interval: Duration,

    /// Maximum number of reconnection attempts
    max_reconnect_attempts: u32,

    /// Connection status
    is_connected: bool,

    /// Close callback
    on_close: Option<CloseCallback>,

    /// Error callback
    on_error: Option<ErrorCallback>,

    /// Message callback
    on_message: Option<MessageCallback>,

    /// Client handle
    client_handle: Option<JoinHandle<()>>,

    /// Store for received messages
    received_messages: Arc<Mutex<Vec<String>>>,

    /// Channel for receiving messages
    message_rx: Option<mpsc::Receiver<String>>,

    /// Sender for the message channel
    message_sender: Arc<mpsc::Sender<String>>,
}

impl SSEClientTransport {
    /// Create a new SSE transport in client mode
    ///
    /// Only requires the event source URL. The send URL will be dynamically
    /// provided by the server via an "endpoint" SSE event according to the MCP protocol.
    pub fn new(event_source_url: &str) -> Result<Self, MCPError> {
        let url = Url::parse(event_source_url)
            .map_err(|e| MCPError::Transport(format!("Invalid event source URL: {}", e)))?;

        // Create HTTP client
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| MCPError::Transport(format!("Failed to create HTTP client: {}", e)))?;

        // Create a channel for receiving messages
        let (message_tx, message_rx) = mpsc::channel::<String>(100);
        let message_sender = Arc::new(message_tx);
        let received_messages = Arc::new(Mutex::new(Vec::new()));

        Ok(Self {
            url,
            send_url: Arc::new(Mutex::new(None)),
            http_client,
            auth_token: None,
            reconnect_interval: Duration::from_secs(3), // Default 3 seconds
            max_reconnect_attempts: 5,                  // Default 5 attempts
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
            client_handle: None,
            received_messages,
            message_rx: Some(message_rx),
            message_sender,
        })
    }

    /// Set authentication token for requests
    pub fn with_auth_token(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set reconnection parameters
    pub fn with_reconnect_params(mut self, interval_secs: u64, max_attempts: u32) -> Self {
        self.reconnect_interval = Duration::from_secs(interval_secs);
        self.max_reconnect_attempts = max_attempts;
        self
    }

    /// Start the SSE client
    async fn start_client(&mut self) -> Result<(), MCPError> {
        if self.is_connected {
            return Ok(());
        }

        // Clone necessary data for the client task
        let url = self.url.clone();
        let client = self.http_client.clone();
        let send_url_mutex = self.send_url.clone();
        let message_sender = self.message_sender.clone();
        let received_messages = self.received_messages.clone();
        let auth_token = self.auth_token.clone();
        let reconnect_interval = self.reconnect_interval;
        let max_reconnect_attempts = self.max_reconnect_attempts;

        // Create and spawn the client task
        let client_task = tokio::spawn(async move {
            let mut attempts = 0;

            'reconnect: loop {
                if attempts >= max_reconnect_attempts {
                    eprintln!("Maximum reconnection attempts reached, giving up");
                    break;
                }

                // Create the request for SSE events
                let mut req = client.get(url.clone());

                // Add SSE headers
                req = req.header("Accept", "text/event-stream");

                // Add auth if available
                if let Some(token) = &auth_token {
                    req = req.header("Authorization", format!("Bearer {}", token));
                }

                debug!("Connecting to SSE endpoint: {}", url);

                // Send the request and get response
                let response = match req.send().await {
                    Ok(res) => {
                        if !res.status().is_success() {
                            eprintln!("Server returned error status: {}", res.status());
                            attempts += 1;
                            tokio::time::sleep(reconnect_interval).await;
                            continue 'reconnect;
                        }
                        res
                    }
                    Err(e) => {
                        eprintln!("Failed to connect to SSE endpoint: {}", e);
                        attempts += 1;
                        tokio::time::sleep(reconnect_interval).await;
                        continue 'reconnect;
                    }
                };

                // Reset attempts counter on successful connection
                attempts = 0;

                // Create an SSE stream from the response
                let mut event_stream = response.bytes_stream().eventsource();

                // Process events
                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            // Get event type
                            let event_type = event.event.as_str();

                            // Process based on event type
                            match event_type {
                                "endpoint" => {
                                    // Update endpoint URL from data
                                    let data = event.data;
                                    if let Ok(endpoint_url) = Url::parse(&data) {
                                        let mut send_url = send_url_mutex.lock().await;
                                        *send_url = Some(endpoint_url);
                                        debug!("Received endpoint URL: {}", data);
                                    } else {
                                        eprintln!("Received invalid endpoint URL: {}", data);
                                    }
                                }
                                "message" | "" => {
                                    // Process message data
                                    let data = event.data;
                                    // Store the message
                                    {
                                        let mut messages = received_messages.lock().await;
                                        messages.push(data.clone());
                                    }

                                    // Send to channel
                                    let _ = message_sender.send(data).await;
                                }
                                _ => {
                                    // Ignore unknown event types
                                    debug!("Received unknown event type: {}", event_type);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error parsing SSE event: {}", e);
                            break;
                        }
                    }
                }

                // If we reach here, the connection was lost
                eprintln!("SSE connection lost, attempting to reconnect...");
                tokio::time::sleep(reconnect_interval).await;
            }
        });

        // Store the handle
        self.client_handle = Some(client_task);
        self.is_connected = true;

        Ok(())
    }

    /// Handle an error by calling the error callback if set
    fn handle_error(&self, error: &MCPError) {
        if let Some(callback) = &self.on_error {
            callback(error);
        }
    }
}

impl Clone for SSEClientTransport {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            send_url: self.send_url.clone(),
            http_client: self.http_client.clone(),
            auth_token: self.auth_token.clone(),
            reconnect_interval: self.reconnect_interval,
            max_reconnect_attempts: self.max_reconnect_attempts,
            is_connected: self.is_connected,
            on_close: None,      // Callbacks are not cloned
            on_error: None,      // Callbacks are not cloned
            on_message: None,    // Callbacks are not cloned
            client_handle: None, // The handle is not cloned
            received_messages: self.received_messages.clone(),
            message_rx: None, // The receiver is not cloned
            message_sender: self.message_sender.clone(),
        }
    }
}

#[async_trait]
impl Transport for SSEClientTransport {
    async fn start(&mut self) -> Result<(), MCPError> {
        self.start_client().await
    }

    async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        // Get the send URL
        let send_url = {
            let url_guard = self.send_url.lock().await;
            match &*url_guard {
                Some(url) => url.clone(),
                None => {
                    let error =
                        MCPError::Transport("Send URL not received from server".to_string());
                    self.handle_error(&error);
                    return Err(error);
                }
            }
        };

        // Serialize the message
        let json = serde_json::to_string(message).map_err(|e| {
            let error = MCPError::Serialization(e.to_string());
            self.handle_error(&error);
            error
        })?;

        // Send the message using reqwest
        let mut req = self.http_client.post(send_url);

        // Add any auth headers
        if let Some(token) = &self.auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        // Send the request
        let response = req
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| {
                let error = MCPError::Transport(format!("Failed to send message: {}", e));
                self.handle_error(&error);
                error
            })?;

        // Check response status
        if !response.status().is_success() {
            let error = MCPError::Transport(format!(
                "Server returned error status: {}",
                response.status()
            ));
            self.handle_error(&error);
            return Err(error);
        }

        Ok(())
    }

    async fn receive<T: DeserializeOwned + Send + Sync>(&mut self) -> Result<T, MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        // Get the message receiver
        let mut message_rx = match self.message_rx.take() {
            Some(rx) => rx,
            None => {
                let error = MCPError::Transport("Message receiver unavailable".to_string());
                self.handle_error(&error);
                return Err(error);
            }
        };

        // Wait for a message
        let json = match message_rx.recv().await {
            Some(json) => {
                // Put the receiver back
                self.message_rx = Some(message_rx);
                json
            }
            None => {
                let error = MCPError::Transport("Message channel closed".to_string());
                self.handle_error(&error);
                return Err(error);
            }
        };

        // Parse the message
        serde_json::from_str(&json).map_err(|e| {
            let error = MCPError::Deserialization(e.to_string());
            self.handle_error(&error);
            error
        })
    }

    async fn close(&mut self) -> Result<(), MCPError> {
        if !self.is_connected {
            return Ok(());
        }

        // Cancel the client task if it's running
        if let Some(handle) = self.client_handle.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Reset state
        self.is_connected = false;

        // Call the close callback
        if let Some(callback) = &self.on_close {
            callback();
        }

        Ok(())
    }

    fn set_on_close(&mut self, callback: Option<CloseCallback>) {
        self.on_close = callback;
    }

    fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
        self.on_error = callback;
    }

    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_message = callback.map(|f| Box::new(f) as MessageCallback);
    }
}

// Additional utility methods
impl SSEClientTransport {
    pub fn has_auth_token(&self) -> bool {
        self.auth_token.is_some()
    }

    pub fn get_auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub fn get_reconnect_interval(&self) -> Duration {
        self.reconnect_interval
    }

    pub fn get_max_reconnect_attempts(&self) -> u32 {
        self.max_reconnect_attempts
    }
}
