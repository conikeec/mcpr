use crate::error::MCPError;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use log::{debug, error, info};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type CallbackHandler = Arc<Mutex<CallbackState>>;

struct CallbackState {
    on_message: Option<MessageCallback>,
    on_error: Option<ErrorCallback>,
    on_close: Option<CloseCallback>,
}

/// TCP transport implementation for MCP
pub struct TcpTransport {
    address: String,
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
    writer: Option<BufWriter<TcpStream>>,
    is_server: bool,
    is_connected: Arc<Mutex<bool>>,
    callbacks: CallbackHandler,
    listener: Option<Arc<TcpListener>>,
    receiver_thread: Option<thread::JoinHandle<()>>,
}

impl Default for TcpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpTransport {
    /// Create a new TCP transport client
    pub fn new() -> Self {
        Self {
            address: String::new(),
            stream: None,
            reader: None,
            writer: None,
            is_server: false,
            is_connected: Arc::new(Mutex::new(false)),
            callbacks: Arc::new(Mutex::new(CallbackState {
                on_message: None,
                on_error: None,
                on_close: None,
            })),
            listener: None,
            receiver_thread: None,
        }
    }

    /// Create a new TCP transport client with a specific address
    pub fn connect(host: &str, port: u16) -> Result<Self, MCPError> {
        let address = format!("{}:{}", host, port);
        info!("Creating TCP transport client for address: {}", address);

        let mut transport = Self::new();
        transport.address = address;

        Ok(transport)
    }

    /// Create a new TCP transport server
    pub fn bind(host: &str, port: u16) -> Result<Self, MCPError> {
        let address = format!("{}:{}", host, port);
        info!("Creating TCP transport server on address: {}", address);

        let mut transport = Self::new();
        transport.address = address;
        transport.is_server = true;

        Ok(transport)
    }

    /// Handle a transport error
    fn handle_error(&self, error: &MCPError) {
        error!("TCP transport error: {}", error);
        if let Ok(callbacks) = self.callbacks.lock() {
            if let Some(callback) = &callbacks.on_error {
                callback(error);
            }
        }
    }

    /// Start TCP client mode
    fn start_client(&mut self) -> Result<(), MCPError> {
        debug!("Starting TCP client for address: {}", self.address);

        // Connect to the server
        let stream = TcpStream::connect(&self.address).map_err(|e| {
            MCPError::Transport(format!("Failed to connect to {}: {}", self.address, e))
        })?;

        // Create a clone of the stream for the reader
        let reader_stream = stream
            .try_clone()
            .map_err(|e| MCPError::Transport(format!("Failed to clone TCP stream: {}", e)))?;

        // Initialize the reader and writer
        self.reader = Some(BufReader::new(reader_stream));
        self.writer = Some(BufWriter::new(stream.try_clone().unwrap()));
        self.stream = Some(stream);

        // Set connected flag
        if let Ok(mut connected) = self.is_connected.lock() {
            *connected = true;
        } else {
            return Err(MCPError::Transport(
                "Failed to access connection state".to_string(),
            ));
        }

        // Start a receiver thread to process incoming messages
        let reader = self
            .reader
            .as_ref()
            .unwrap()
            .get_ref()
            .try_clone()
            .map_err(|e| MCPError::Transport(format!("Failed to clone reader stream: {}", e)))?;

        let is_connected_clone = Arc::clone(&self.is_connected);
        let callbacks_clone = Arc::clone(&self.callbacks);

        let receiver_thread = thread::spawn(move || {
            debug!("TCP client receiver thread started");

            let mut reader = BufReader::new(reader);

            loop {
                // Check if we should exit
                let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                    !*connected
                } else {
                    true
                };

                if should_exit {
                    debug!("TCP client receiver thread exiting");
                    break;
                }

                // Read a line from the stream
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        // End of stream, connection closed
                        debug!("TCP connection closed by peer");
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
                    Ok(_) => {
                        // Trim whitespace and newlines
                        let message = line.trim().to_string();
                        debug!("Received TCP message: {}", message);

                        // Call the message callback
                        if let Ok(callbacks) = callbacks_clone.lock() {
                            if let Some(callback) = &callbacks.on_message {
                                callback(&message);
                            }
                        }
                    }
                    Err(e) => {
                        // Error reading from stream
                        let error =
                            MCPError::Transport(format!("Error reading from TCP stream: {}", e));
                        error!("{}", error);
                        if let Ok(callbacks) = callbacks_clone.lock() {
                            if let Some(callback) = &callbacks.on_error {
                                callback(&error);
                            }
                        }

                        // Close the connection
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

            debug!("TCP client receiver thread exited");
        });

        self.receiver_thread = Some(receiver_thread);
        info!("TCP client started successfully");
        Ok(())
    }

    /// Start TCP server mode
    fn start_server(&mut self) -> Result<(), MCPError> {
        debug!("Starting TCP server on address: {}", self.address);

        // Create a TCP listener
        let listener = TcpListener::bind(&self.address).map_err(|e| {
            MCPError::Transport(format!("Failed to bind to {}: {}", self.address, e))
        })?;

        // Store the listener
        let listener_arc = Arc::new(listener);
        self.listener = Some(Arc::clone(&listener_arc));

        // Set server as "connected" (ready to accept connections)
        if let Ok(mut connected) = self.is_connected.lock() {
            *connected = true;
        } else {
            return Err(MCPError::Transport(
                "Failed to access connection state".to_string(),
            ));
        }

        // Start a thread to accept incoming connections
        let is_connected_clone = Arc::clone(&self.is_connected);
        let callbacks_clone = Arc::clone(&self.callbacks);

        let server_thread = thread::spawn(move || {
            debug!("TCP server accepting thread started");

            // Set listener to non-blocking mode
            if let Ok(_listener) = listener_arc.set_nonblocking(true) {
                debug!("TCP server set to non-blocking mode");
            } else {
                error!("Failed to set TCP server to non-blocking mode");
            }

            // Accept incoming connections
            while let Ok(connected) = is_connected_clone.lock() {
                if !*connected {
                    debug!("TCP server shutting down");
                    break;
                }

                // Try to accept a connection
                match listener_arc.accept() {
                    Ok((stream, addr)) => {
                        debug!("TCP connection accepted from {}", addr);

                        // For simplicity, we only handle one client at a time for now
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let _writer = BufWriter::new(stream);

                        // Process messages from this client
                        loop {
                            // Check if we should exit
                            let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                                !*connected
                            } else {
                                true
                            };

                            if should_exit {
                                debug!("TCP server thread exiting");
                                break;
                            }

                            // Read a line from the client
                            let mut line = String::new();
                            match reader.read_line(&mut line) {
                                Ok(0) => {
                                    // End of stream, connection closed
                                    debug!("TCP connection closed by client");
                                    break;
                                }
                                Ok(_) => {
                                    // Trim whitespace and newlines
                                    let message = line.trim().to_string();
                                    debug!("Received TCP message from client: {}", message);

                                    // Call the message callback
                                    if let Ok(callbacks) = callbacks_clone.lock() {
                                        if let Some(callback) = &callbacks.on_message {
                                            callback(&message);
                                        }
                                    }
                                }
                                Err(e) => {
                                    if e.kind() == std::io::ErrorKind::WouldBlock {
                                        // Non-blocking mode, no data available yet
                                        thread::sleep(Duration::from_millis(10));
                                        continue;
                                    }

                                    // Error reading from stream
                                    let error = MCPError::Transport(format!(
                                        "Error reading from client: {}",
                                        e
                                    ));
                                    error!("{}", error);
                                    if let Ok(callbacks) = callbacks_clone.lock() {
                                        if let Some(callback) = &callbacks.on_error {
                                            callback(&error);
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            // Non-blocking mode, no connection available yet
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }

                        // Error accepting connection
                        let error =
                            MCPError::Transport(format!("Error accepting TCP connection: {}", e));
                        error!("{}", error);
                        if let Ok(callbacks) = callbacks_clone.lock() {
                            if let Some(callback) = &callbacks.on_error {
                                callback(&error);
                            }
                        }
                    }
                }
            }

            debug!("TCP server thread exited");
        });

        self.receiver_thread = Some(server_thread);
        info!("TCP server started successfully");
        Ok(())
    }
}

impl Transport for TcpTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        // Check if already connected
        if let Ok(connected) = self.is_connected.lock() {
            if *connected {
                debug!("TCP transport already started");
                return Err(MCPError::AlreadyConnected);
            }
        }

        // Start in client or server mode
        if self.is_server {
            self.start_server()
        } else {
            self.start_client()
        }
    }

    fn close(&mut self) -> Result<(), MCPError> {
        // Check if already disconnected
        if let Ok(connected) = self.is_connected.lock() {
            if !*connected {
                debug!("TCP transport already closed");
                return Ok(());
            }
        }

        info!("Closing TCP transport for address: {}", self.address);

        // Set disconnected flag
        if let Ok(mut connected) = self.is_connected.lock() {
            *connected = false;
        }

        // Close the connection
        self.stream = None;
        self.reader = None;
        self.writer = None;
        self.listener = None;

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

        info!("TCP transport closed successfully");
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

        debug!("Sending JSON via TCP: {}", json_string);

        // Send the message
        if let Some(writer) = &mut self.writer {
            match writeln!(writer, "{}", json_string) {
                Ok(_) => {
                    // Flush the writer to ensure the message is sent immediately
                    match writer.flush() {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            let error =
                                MCPError::Transport(format!("Failed to flush TCP writer: {}", e));
                            self.handle_error(&error);
                            Err(error)
                        }
                    }
                }
                Err(e) => {
                    let error = MCPError::Transport(format!("Failed to send TCP message: {}", e));
                    self.handle_error(&error);
                    Err(error)
                }
            }
        } else {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            Err(error)
        }
    }

    fn receive_json(&mut self) -> Result<String, MCPError> {
        // Check if connected
        if let Ok(connected) = self.is_connected.lock() {
            if !*connected {
                let error = MCPError::NotConnected;
                self.handle_error(&error);
                return Err(error);
            }
        }

        // In TCP, we use the receiver thread for async handling
        // This function is mainly for compatibility, but not efficient for TCP
        if let Some(reader) = &mut self.reader {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // End of stream, connection closed
                    let error = MCPError::Transport("TCP connection closed by peer".to_string());
                    self.handle_error(&error);
                    Err(error)
                }
                Ok(_) => {
                    // Trim whitespace and newlines
                    let message = line.trim().to_string();
                    debug!("Received TCP message: {}", message);
                    Ok(message)
                }
                Err(e) => {
                    let error =
                        MCPError::Transport(format!("Error reading from TCP stream: {}", e));
                    self.handle_error(&error);
                    Err(error)
                }
            }
        } else {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            Err(error)
        }
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
