use crate::error::MCPError;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use log::{debug, error, info};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use url::Url;

type CallbackHandler = Arc<Mutex<CallbackState>>;

struct CallbackState {
    on_message: Option<MessageCallback>,
    on_error: Option<ErrorCallback>,
    on_close: Option<CloseCallback>,
}

/// WebSocket transport implementation for MCP
pub struct WebSocketTransport {
    url: String,
    socket: Arc<Mutex<Option<WebSocket<MaybeTlsStream<std::net::TcpStream>>>>>,
    is_connected: Arc<Mutex<bool>>,
    callbacks: CallbackHandler,
    receiver_thread: Option<thread::JoinHandle<()>>,
}

impl WebSocketTransport {
    /// Create a new WebSocket transport
    pub fn new(url: &str) -> Self {
        info!("Creating new WebSocket transport with URL: {}", url);
        Self {
            url: url.to_string(),
            socket: Arc::new(Mutex::new(None)),
            is_connected: Arc::new(Mutex::new(false)),
            callbacks: Arc::new(Mutex::new(CallbackState {
                on_message: None,
                on_error: None,
                on_close: None,
            })),
            receiver_thread: None,
        }
    }

    /// Handle a transport error
    fn handle_error(&self, error: &MCPError) {
        error!("WebSocket transport error: {}", error);
        if let Ok(callbacks) = self.callbacks.lock() {
            if let Some(callback) = &callbacks.on_error {
                callback(error);
            }
        }
    }
}

impl Transport for WebSocketTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        // Check if already connected
        if let Ok(connected) = self.is_connected.lock() {
            if *connected {
                debug!("WebSocket transport already connected");
                return Err(MCPError::AlreadyConnected);
            }
        }

        info!("Starting WebSocket transport with URL: {}", self.url);

        // Parse URL
        let url = Url::parse(&self.url)
            .map_err(|e| MCPError::Transport(format!("Invalid WebSocket URL: {}", e)))?;

        // Connect to the WebSocket server
        let (socket, _) = connect(url)
            .map_err(|e| MCPError::Transport(format!("Failed to connect to WebSocket: {}", e)))?;

        info!("Successfully connected to WebSocket server");

        // Store the socket
        if let Ok(mut socket_guard) = self.socket.lock() {
            *socket_guard = Some(socket);
        } else {
            return Err(MCPError::Transport(
                "Failed to access socket mutex".to_string(),
            ));
        }

        // Set connected flag
        if let Ok(mut connected) = self.is_connected.lock() {
            *connected = true;
        } else {
            return Err(MCPError::Transport(
                "Failed to access connection state".to_string(),
            ));
        }

        // Start a receiver thread to process incoming messages
        let socket_clone = Arc::clone(&self.socket);
        let is_connected_clone = Arc::clone(&self.is_connected);
        let callbacks_clone = Arc::clone(&self.callbacks);

        let receiver_thread = thread::spawn(move || {
            debug!("WebSocket receiver thread started");

            loop {
                // Check if we should exit
                let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                    !*connected
                } else {
                    true
                };

                if should_exit {
                    debug!("WebSocket receiver thread exiting");
                    break;
                }

                // Get access to the socket
                if let Ok(mut socket_guard) = socket_clone.lock() {
                    if let Some(socket) = &mut *socket_guard {
                        // Try to receive a message
                        match socket.read() {
                            Ok(message) => {
                                match message {
                                    Message::Text(text) => {
                                        debug!("Received WebSocket text message: {}", text);
                                        if let Ok(callbacks) = callbacks_clone.lock() {
                                            if let Some(callback) = &callbacks.on_message {
                                                callback(&text);
                                            }
                                        }
                                    }
                                    Message::Binary(data) => {
                                        debug!(
                                            "Received WebSocket binary message ({} bytes)",
                                            data.len()
                                        );
                                        // Binary messages not supported by MCP
                                    }
                                    Message::Ping(_) => {
                                        // Automatically handled by tungstenite
                                        debug!("Received WebSocket ping");
                                    }
                                    Message::Pong(_) => {
                                        debug!("Received WebSocket pong");
                                    }
                                    Message::Close(_) => {
                                        debug!("Received WebSocket close frame");
                                        if let Ok(mut connected) = is_connected_clone.lock() {
                                            *connected = false;
                                        }
                                        if let Ok(callbacks) = callbacks_clone.lock() {
                                            if let Some(callback) = &callbacks.on_close {
                                                callback();
                                            }
                                        }
                                        break;
                                    }
                                    Message::Frame(_) => {
                                        // Raw frames not handled by this transport
                                    }
                                }
                            }
                            Err(e) => {
                                let error = MCPError::Transport(format!("WebSocket error: {}", e));
                                error!("{}", error);
                                if let Ok(callbacks) = callbacks_clone.lock() {
                                    if let Some(callback) = &callbacks.on_error {
                                        callback(&error);
                                    }
                                }

                                // Check if this is a fatal error that should close the connection
                                // ConnectionClosed and similar errors are fatal
                                let is_fatal = match e {
                                    tungstenite::Error::ConnectionClosed
                                    | tungstenite::Error::AlreadyClosed
                                    | tungstenite::Error::Tls(_)
                                    | tungstenite::Error::Protocol(_)
                                    | tungstenite::Error::Utf8
                                    | tungstenite::Error::AttackAttempt
                                    | tungstenite::Error::Url(_)
                                    | tungstenite::Error::Http(_)
                                    | tungstenite::Error::HttpFormat(_) => true,
                                    tungstenite::Error::Capacity(_)
                                    | tungstenite::Error::WriteBufferFull(_) => false,
                                    tungstenite::Error::Io(ref io_err) => {
                                        // WouldBlock is not fatal
                                        io_err.kind() != std::io::ErrorKind::WouldBlock
                                    }
                                };

                                if is_fatal {
                                    if let Ok(mut connected) = is_connected_clone.lock() {
                                        *connected = false;
                                    }
                                    if let Ok(callbacks) = callbacks_clone.lock() {
                                        if let Some(callback) = &callbacks.on_close {
                                            callback();
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    } else {
                        // No socket available
                        break;
                    }
                } else {
                    // Can't access socket
                    break;
                }

                // Short sleep to avoid consuming too much CPU
                thread::sleep(Duration::from_millis(10));
            }

            debug!("WebSocket receiver thread exited");
        });

        self.receiver_thread = Some(receiver_thread);
        Ok(())
    }

    fn close(&mut self) -> Result<(), MCPError> {
        // Check if already disconnected
        if let Ok(connected) = self.is_connected.lock() {
            if !*connected {
                debug!("WebSocket transport already closed");
                return Ok(());
            }
        }

        info!("Closing WebSocket transport for URL: {}", self.url);

        // Set disconnected flag
        if let Ok(mut connected) = self.is_connected.lock() {
            *connected = false;
        }

        // Close the socket with a normal closure
        if let Ok(mut socket_guard) = self.socket.lock() {
            if let Some(socket) = &mut *socket_guard {
                if let Err(e) = socket.close(None) {
                    error!("Error closing WebSocket connection: {}", e);
                }
            }
        }

        // Wait for the receiver thread to exit
        if let Some(thread) = self.receiver_thread.take() {
            // Join with a timeout to avoid blocking indefinitely
            let _ = thread.join();
        }

        // Call the close callback
        if let Ok(callbacks) = self.callbacks.lock() {
            if let Some(callback) = &callbacks.on_close {
                callback();
            }
        }

        info!("WebSocket transport closed successfully");
        Ok(())
    }

    fn send_json(&mut self, json_string: &str) -> Result<(), MCPError> {
        // Check if connected
        if let Ok(connected) = self.is_connected.lock() {
            if !*connected {
                let error = MCPError::NotConnected;
                self.handle_error(&error);
                return Err(error);
            }
        }

        debug!("Sending JSON via WebSocket: {}", json_string);

        // Send the message
        if let Ok(mut socket_guard) = self.socket.lock() {
            if let Some(socket) = &mut *socket_guard {
                match socket.send(Message::Text(json_string.to_string())) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        let error =
                            MCPError::Transport(format!("Failed to send WebSocket message: {}", e));
                        self.handle_error(&error);
                        Err(error)
                    }
                }
            } else {
                let error = MCPError::NotConnected;
                self.handle_error(&error);
                Err(error)
            }
        } else {
            let error = MCPError::Transport("Failed to access socket mutex".to_string());
            self.handle_error(&error);
            Err(error)
        }
    }

    fn receive_json(&mut self) -> Result<String, MCPError> {
        // WebSockets use async handling via the receiver thread
        // This function is not expected to be called for WebSocket transport
        Err(MCPError::Transport(
            "WebSocket transport uses async message handling".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        if let Ok(connected) = self.is_connected.lock() {
            *connected
        } else {
            false
        }
    }

    fn on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.on_message = Some(callback);
        }
    }

    fn on_error(&mut self, callback: Box<dyn Fn(&MCPError) + Send + Sync>) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.on_error = Some(callback);
        }
    }

    fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.on_close = Some(callback);
        }
    }

    fn set_on_close(&mut self, callback: Option<CloseCallback>) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.on_close = callback;
        }
    }

    fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.on_error = callback;
        }
    }
}
