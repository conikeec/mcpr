use crate::error::MCPError;
use crate::transport::sse::session::SessionManager;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use url::Url;
use uuid::Uuid;
use warp::Filter;

use super::sse_stream::SseEventStream;

/// Server-Sent Events (SSE) Server Transport
pub struct SSEServerTransport {
    /// The base URL for the server
    url: Url,

    /// Connection status
    is_connected: bool,

    /// Message queue for sending
    sender_tx: mpsc::Sender<String>,

    /// Close callback
    on_close: Option<CloseCallback>,

    /// Error callback
    on_error: Option<ErrorCallback>,

    /// Message callback
    on_message: Option<MessageCallback>,

    /// Server handle
    server_handle: Option<JoinHandle<()>>,

    /// Session manager
    session_manager: SessionManager,

    /// Shutdown channel
    server_shutdown_tx: Option<mpsc::Sender<()>>,

    /// Store for received messages
    received_messages: Arc<Mutex<Vec<String>>>,

    /// Channel for receiving messages
    message_rx: Option<mpsc::Receiver<String>>,

    /// Sender for the message channel
    message_sender: Arc<mpsc::Sender<String>>,
}

impl SSEServerTransport {
    /// Create a new SSE transport in server mode
    pub fn new(url: &str) -> Result<Self, MCPError> {
        let url = Url::parse(url)
            .map_err(|e| MCPError::Transport(format!("Invalid server URL: {}", e)))?;

        // Create a sender channel for server message sending
        let (sender_tx, _) = mpsc::channel::<String>(32);

        // Create a broadcast channel for SSE events
        let (broadcast_tx, _) = broadcast::channel::<String>(100);
        let broadcaster = Arc::new(broadcast_tx);

        // Create channel for receiving messages
        let (message_tx, message_rx) = mpsc::channel::<String>(100);
        let message_sender = Arc::new(message_tx);
        let received_messages = Arc::new(Mutex::new(Vec::new()));

        // Create session manager
        let session_manager = SessionManager::new(broadcaster);

        Ok(Self {
            url,
            is_connected: false,
            sender_tx,
            on_close: None,
            on_error: None,
            on_message: None,
            server_handle: None,
            session_manager,
            server_shutdown_tx: None,
            received_messages,
            message_rx: Some(message_rx),
            message_sender,
        })
    }

    /// Start the SSE server
    async fn start_server(&mut self) -> Result<(), MCPError> {
        if self.is_connected {
            return Ok(());
        }

        // Get the host and port from the URL
        let host = self.url.host_str().unwrap_or("127.0.0.1");
        let port = self.url.port().unwrap_or(8000);
        let addr = format!("{}:{}", host, port)
            .parse::<SocketAddr>()
            .map_err(|e| MCPError::Transport(format!("Invalid address: {}", e)))?;

        // Create a channel for shutdown signaling
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.server_shutdown_tx = Some(shutdown_tx);

        // Clone necessary data for the server task
        let session_manager = self.session_manager.clone();
        let received_messages = self.received_messages.clone();
        let message_sender = self.message_sender.clone();

        // Create SSE route for events
        let sse_session_manager = session_manager.clone();
        let sse_route = warp::path("events")
            .and(warp::get())
            .and(warp::query::<SessionQuery>())
            .map(move |query: SessionQuery| {
                let session_mgr = sse_session_manager.clone();
                let session_id = query
                    .session_id
                    .unwrap_or_else(|| Uuid::new_v4().to_string());

                // Create response with SSE stream
                warp::reply::with_header(
                    warp::reply::Response::new(warp::hyper::Body::wrap_stream(
                        SseEventStream::new(session_mgr, session_id),
                    )),
                    "content-type",
                    "text/event-stream",
                )
            });

        // Create messages route for client -> server communication
        let messages_session_manager = session_manager.clone();
        let messages_route = warp::path("messages")
            .and(warp::post())
            .and(warp::query::<SessionQuery>())
            .and(warp::body::content_length_limit(1024 * 16))
            .and(warp::body::json())
            .and(with_data(received_messages.clone()))
            .and(with_data(message_sender.clone()))
            .and(with_data(messages_session_manager.clone()))
            .and_then(handle_message);

        // Combine routes with CORS support
        let routes = sse_route
            .with(warp::cors().allow_any_origin())
            .or(messages_route);

        // Start the server
        let (addr, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, async move {
            let _ = shutdown_rx.recv().await;
            println!("SSE server shutting down");
        });

        // Spawn the server as a separate task
        let handle = tokio::spawn(async move {
            println!("SSE server listening on http://{}", addr);
            println!("Endpoints:");
            println!("  - GET  http://{}/events   (SSE events stream)", addr);
            println!("  - POST http://{}/messages (Message endpoint)", addr);
            server.await;
        });

        self.server_handle = Some(handle);
        self.is_connected = true;

        Ok(())
    }

    /// Handle an error by calling the error callback if set
    fn handle_error(&self, error: &MCPError) {
        if let Some(callback) = &self.on_error {
            callback(error);
        }
    }

    /// Broadcast a message to all SSE clients
    pub async fn broadcast<T: Serialize + Send + Sync>(&self, message: &T) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        let json = serde_json::to_string(message).map_err(|e| {
            let error = MCPError::Serialization(e.to_string());
            self.handle_error(&error);
            error
        })?;

        // Broadcast the message
        if let Err(e) = self.session_manager.broadcast(&json) {
            let error = MCPError::Transport(e);
            self.handle_error(&error);
            return Err(error);
        }

        Ok(())
    }

    /// Send a response to a specific client session
    pub async fn send_to_session<T: Serialize + Send + Sync>(
        &self,
        session_id: &str,
        message: &T,
    ) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        let json = serde_json::to_string(message).map_err(|e| {
            let error = MCPError::Serialization(e.to_string());
            self.handle_error(&error);
            error
        })?;

        // Send to the specific session
        match self
            .session_manager
            .send_to_session(session_id, &json)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                let error = MCPError::Transport(e);
                self.handle_error(&error);
                Err(error)
            }
        }
    }
}

impl Clone for SSEServerTransport {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            is_connected: self.is_connected,
            sender_tx: self.sender_tx.clone(),
            on_close: None,      // Callbacks are not cloned
            on_error: None,      // Callbacks are not cloned
            on_message: None,    // Callbacks are not cloned
            server_handle: None, // The handle is not cloned
            session_manager: self.session_manager.clone(),
            server_shutdown_tx: None, // The shutdown channel is not cloned
            received_messages: self.received_messages.clone(),
            message_rx: None, // The receiver is not cloned
            message_sender: self.message_sender.clone(),
        }
    }
}

#[async_trait]
impl Transport for SSEServerTransport {
    async fn start(&mut self) -> Result<(), MCPError> {
        self.start_server().await
    }

    async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError> {
        // For server, broadcast the message to all clients
        self.broadcast(message).await
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

        // Signal server shutdown
        if let Some(tx) = &self.server_shutdown_tx {
            let _ = tx.send(()).await;
        }

        // Wait for server to shut down
        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }

        // Reset state
        self.is_connected = false;
        self.server_shutdown_tx = None;

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

/// Session query struct for SSE connections
#[derive(Debug, serde::Deserialize)]
struct SessionQuery {
    session_id: Option<String>,
}

/// Handle incoming messages from clients
async fn handle_message(
    query: SessionQuery,
    message: serde_json::Value,
    messages: Arc<Mutex<Vec<String>>>,
    message_tx: Arc<mpsc::Sender<String>>,
    session_manager: SessionManager,
) -> Result<impl warp::Reply, Infallible> {
    let session_id = query.session_id;

    // Convert message to string
    let msg_str = message.to_string();

    // Store the message
    {
        let mut store = messages.lock().await;
        store.push(msg_str.clone());
    }

    // Send to message channel
    let _ = message_tx.send(msg_str.clone()).await;

    // If a session ID was provided, try to send to that specific session
    if let Some(id) = session_id {
        if session_manager.session_exists(&id).await {
            let _ = session_manager.send_to_session(&id, &msg_str).await;
        }
    }

    Ok(warp::reply::json(&serde_json::json!({
        "status": "success",
        "message": "Message received"
    })))
}

/// Helper filter to extract data
fn with_data<T: Clone + Send>(data: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || data.clone())
}
