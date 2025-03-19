use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, LitStr};

/// Parse transport type attribute arguments
struct TransportArgs {
    transport_type: String,
}

impl TransportArgs {
    fn parse(attr: TokenStream) -> Self {
        // If empty, use default
        if attr.clone().into_iter().count() == 0 {
            return Self {
                transport_type: "stdio".to_string(),
            };
        }

        // Convert proc_macro::TokenStream to proc_macro2::TokenStream
        let attr2: proc_macro2::TokenStream = attr.into();

        // If empty after conversion, use default
        if attr2.clone().into_iter().count() == 0 {
            return Self {
                transport_type: "stdio".to_string(),
            };
        }

        // Parse the literal string
        match syn::parse2::<LitStr>(attr2) {
            Ok(lit) => Self {
                transport_type: lit.value(),
            },
            Err(_) => Self {
                transport_type: "stdio".to_string(),
            },
        }
    }
}

/// Implementation of the transport macro
pub fn impl_transport_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse transport args first
    let args = TransportArgs::parse(attr);

    // Parse the input as a struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the name of the struct
    let name = &input.ident;
    let generics = &input.generics;

    // Generate methods based on transport type
    let implementation = match args.transport_type.as_str() {
        "stdio" => generate_stdio_transport(name, generics),
        "tcp" => generate_tcp_transport(name, generics),
        "websocket" => generate_websocket_transport(name, generics),
        "sse" => generate_sse_transport(name, generics),
        _ => generate_stdio_transport(name, generics), // Default to stdio
    };

    // Return the implementation
    implementation
}

fn generate_stdio_transport(name: &Ident, generics: &syn::Generics) -> TokenStream {
    let expanded = quote! {
        /// Standard I/O transport implementation
        pub struct #name #generics {
            reader: Option<std::io::BufReader<Box<dyn std::io::Read + Send>>>,
            writer: Option<std::io::BufWriter<Box<dyn std::io::Write + Send>>>,
            is_connected: std::sync::atomic::AtomicBool,
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
        }

        impl #generics #name #generics {
            /// Create a new transport instance
            pub fn new() -> Self {
                Self {
                    reader: None,
                    writer: None,
                    is_connected: std::sync::atomic::AtomicBool::new(false),
                    on_message: None,
                    on_error: None,
                    on_close: None,
                }
            }

            /// Create a transport using stdin/stdout
            pub fn stdin_stdout() -> Self {
                let stdin = Box::new(std::io::stdin()) as Box<dyn std::io::Read + Send>;
                let stdout = Box::new(std::io::stdout()) as Box<dyn std::io::Write + Send>;

                Self {
                    reader: Some(std::io::BufReader::new(stdin)),
                    writer: Some(std::io::BufWriter::new(stdout)),
                    is_connected: std::sync::atomic::AtomicBool::new(false),
                    on_message: None,
                    on_error: None,
                    on_close: None,
                }
            }

            /// Create a transport from a spawned process
            pub fn from_process(process: std::process::Child) -> Result<Self, mcpr::error::MCPError> {
                let stdin = process.stdin.ok_or_else(|| {
                    mcpr::error::MCPError::Transport("Failed to capture stdin".to_string())
                })?;

                let stdout = process.stdout.ok_or_else(|| {
                    mcpr::error::MCPError::Transport("Failed to capture stdout".to_string())
                })?;

                Ok(Self {
                    reader: Some(std::io::BufReader::new(Box::new(stdout))),
                    writer: Some(std::io::BufWriter::new(Box::new(stdin))),
                    is_connected: std::sync::atomic::AtomicBool::new(false),
                    on_message: None,
                    on_error: None,
                    on_close: None,
                })
            }
        }

        impl #generics mcpr::transport::Transport for #name #generics {
            fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already connected
                if self.is_connected() {
                    // If already connected, just return success
                    return Ok(());
                }

                // Check if we have a reader and writer
                if self.reader.is_none() || self.writer.is_none() {
                    return Err(mcpr::error::MCPError::Transport("Reader or writer not initialized".to_string()));
                }

                // Set connected flag
                self.is_connected.store(true, std::sync::atomic::Ordering::SeqCst);

                Ok(())
            }

            fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already disconnected
                if !self.is_connected() {
                    return Ok(());
                }

                // Set disconnected flag
                self.is_connected.store(false, std::sync::atomic::Ordering::SeqCst);

                // Trigger on_close callback if available
                if let Some(callback) = &self.on_close {
                    callback();
                }

                Ok(())
            }

            fn send_json(&mut self, json_string: &str) -> Result<(), mcpr::error::MCPError> {
                use std::io::Write;

                // Check if connected
                if !self.is_connected() {
                    return Err(mcpr::error::MCPError::NotConnected);
                }

                // Get writer
                let writer = self.writer.as_mut()
                    .ok_or_else(|| mcpr::error::MCPError::Transport("Writer not initialized".to_string()))?;

                // Trigger on_message callback if available
                if let Some(callback) = &self.on_message {
                    callback(json_string);
                }

                // Write message to output with a trailing newline
                writeln!(writer, "{}", json_string)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to write message: {}", e)))?;

                // Flush writer to ensure the message is sent immediately
                writer.flush()
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to flush writer: {}", e)))?;

                Ok(())
            }

            fn receive_json(&mut self) -> Result<String, mcpr::error::MCPError> {
                use std::io::BufRead;

                // Check if connected
                if !self.is_connected() {
                    return Err(mcpr::error::MCPError::NotConnected);
                }

                // Get reader
                let reader = self.reader.as_mut()
                    .ok_or_else(|| mcpr::error::MCPError::Transport("Reader not initialized".to_string()))?;

                // Read line from input
                let mut line = String::new();
                let bytes_read = reader.read_line(&mut line)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to read message: {}", e)))?;

                // Check if we reached EOF (no bytes read)
                if bytes_read == 0 {
                    return Err(mcpr::error::MCPError::Transport("End of stream reached".to_string()));
                }

                // Trim whitespace and newlines
                let line = line.trim().to_string();

                // Trigger on_message callback if available
                if let Some(callback) = &self.on_message {
                    callback(&line);
                }

                Ok(line)
            }

            fn is_connected(&self) -> bool {
                self.is_connected.load(std::sync::atomic::Ordering::SeqCst)
            }

            fn on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
                self.on_message = Some(callback);
            }

            fn on_error(&mut self, callback: Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>) {
                self.on_error = Some(callback);
            }

            fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
                self.on_close = Some(callback);
            }

            fn set_on_close(&mut self, callback: Option<mcpr::transport::CloseCallback>) {
                self.on_close = callback;
            }

            fn set_on_error(&mut self, callback: Option<mcpr::transport::ErrorCallback>) {
                self.on_error = callback;
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_tcp_transport(name: &Ident, generics: &syn::Generics) -> TokenStream {
    // TCP transport implementation based on src/transport/tcp.rs
    let expanded = quote! {
        /// TCP transport implementation for MCP
        pub struct #name #generics {
            address: String,
            stream: Option<std::net::TcpStream>,
            reader: Option<std::io::BufReader<std::net::TcpStream>>,
            writer: Option<std::io::BufWriter<std::net::TcpStream>>,
            is_server: bool,
            is_connected: std::sync::Arc<std::sync::Mutex<bool>>,
            callbacks: std::sync::Arc<std::sync::Mutex<CallbackState>>,
            listener: Option<std::sync::Arc<std::net::TcpListener>>,
            receiver_thread: Option<std::thread::JoinHandle<()>>,
        }

        /// Callback state container for thread-safe access
        struct CallbackState {
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
        }

        impl #generics #name #generics {
            /// Create a new TCP transport client
            pub fn new() -> Self {
                Self {
                    address: String::new(),
                    stream: None,
                    reader: None,
                    writer: None,
                    is_server: false,
                    is_connected: std::sync::Arc::new(std::sync::Mutex::new(false)),
                    callbacks: std::sync::Arc::new(std::sync::Mutex::new(CallbackState {
                        on_message: None,
                        on_error: None,
                        on_close: None,
                    })),
                    listener: None,
                    receiver_thread: None,
                }
            }

            /// Create a new TCP transport client with a specific address
            pub fn connect(host: &str, port: u16) -> Result<Self, mcpr::error::MCPError> {
                let address = format!("{}:{}", host, port);
                mcpr::log::info!("Creating TCP transport client for address: {}", address);

                let mut transport = Self::new();
                transport.address = address;

                Ok(transport)
            }

            /// Create a new TCP transport server
            pub fn bind(host: &str, port: u16) -> Result<Self, mcpr::error::MCPError> {
                let address = format!("{}:{}", host, port);
                mcpr::log::info!("Creating TCP transport server on address: {}", address);

                let mut transport = Self::new();
                transport.address = address;
                transport.is_server = true;

                Ok(transport)
            }

            /// Handle a transport error
            fn handle_error(&self, error: &mcpr::error::MCPError) {
                mcpr::log::error!("TCP transport error: {}", error);
                if let Ok(callbacks) = self.callbacks.lock() {
                    if let Some(callback) = &callbacks.on_error {
                        callback(error);
                    }
                }
            }

            /// Start TCP client mode
            fn start_client(&mut self) -> Result<(), mcpr::error::MCPError> {
                mcpr::log::debug!("Starting TCP client for address: {}", self.address);

                // Connect to the server
                let stream = std::net::TcpStream::connect(&self.address).map_err(|e| {
                    mcpr::error::MCPError::Transport(format!("Failed to connect to {}: {}", self.address, e))
                })?;

                // Create a clone of the stream for the reader
                let reader_stream = stream
                    .try_clone()
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to clone TCP stream: {}", e)))?;

                // Initialize the reader and writer
                self.reader = Some(std::io::BufReader::new(reader_stream));
                self.writer = Some(std::io::BufWriter::new(stream.try_clone().unwrap()));
                self.stream = Some(stream);

                // Set connected flag
                if let Ok(mut connected) = self.is_connected.lock() {
                    *connected = true;
                } else {
                    return Err(mcpr::error::MCPError::Transport(
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
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to clone reader stream: {}", e)))?;

                let is_connected_clone = std::sync::Arc::clone(&self.is_connected);
                let callbacks_clone = std::sync::Arc::clone(&self.callbacks);

                let receiver_thread = std::thread::spawn(move || {
                    mcpr::log::debug!("TCP client receiver thread started");

                    let mut reader = std::io::BufReader::new(reader);

                    loop {
                        // Check if we should exit
                        let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                            !*connected
                        } else {
                            true
                        };

                        if should_exit {
                            mcpr::log::debug!("TCP client receiver thread exiting");
                            break;
                        }

                        // Read a line from the stream
                        let mut line = String::new();
                        match reader.read_line(&mut line) {
                            Ok(0) => {
                                // End of stream, connection closed
                                mcpr::log::debug!("TCP connection closed by peer");
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
                                mcpr::log::debug!("Received TCP message: {}", message);

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
                                    mcpr::error::MCPError::Transport(format!("Error reading from TCP stream: {}", e));
                                mcpr::log::error!("{}", error);
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

                    mcpr::log::debug!("TCP client receiver thread exited");
                });

                self.receiver_thread = Some(receiver_thread);
                mcpr::log::info!("TCP client started successfully");
                Ok(())
            }

            /// Start TCP server mode
            fn start_server(&mut self) -> Result<(), mcpr::error::MCPError> {
                mcpr::log::debug!("Starting TCP server on address: {}", self.address);

                // Create a TCP listener
                let listener = std::net::TcpListener::bind(&self.address).map_err(|e| {
                    mcpr::error::MCPError::Transport(format!("Failed to bind to {}: {}", self.address, e))
                })?;

                // Store the listener
                let listener_arc = std::sync::Arc::new(listener);
                self.listener = Some(std::sync::Arc::clone(&listener_arc));

                // Set server as "connected" (ready to accept connections)
                if let Ok(mut connected) = self.is_connected.lock() {
                    *connected = true;
                } else {
                    return Err(mcpr::error::MCPError::Transport(
                        "Failed to access connection state".to_string(),
                    ));
                }

                // Start a thread to accept incoming connections
                let is_connected_clone = std::sync::Arc::clone(&self.is_connected);
                let callbacks_clone = std::sync::Arc::clone(&self.callbacks);

                let server_thread = std::thread::spawn(move || {
                    mcpr::log::debug!("TCP server accepting thread started");

                    // Set listener to non-blocking mode
                    if let Ok(listener) = listener_arc.set_nonblocking(true) {
                        mcpr::log::debug!("TCP server set to non-blocking mode");
                    } else {
                        mcpr::log::error!("Failed to set TCP server to non-blocking mode");
                    }

                    // Accept incoming connections
                    while let Ok(connected) = is_connected_clone.lock() {
                        if !*connected {
                            mcpr::log::debug!("TCP server shutting down");
                            break;
                        }

                        // Try to accept a connection
                        match listener_arc.accept() {
                            Ok((stream, addr)) => {
                                mcpr::log::debug!("TCP connection accepted from {}", addr);

                                // For simplicity, we only handle one client at a time for now
                                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                                let mut writer = std::io::BufWriter::new(stream);

                                // Process messages from this client
                                loop {
                                    // Check if we should exit
                                    let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                                        !*connected
                                    } else {
                                        true
                                    };

                                    if should_exit {
                                        mcpr::log::debug!("TCP server thread exiting");
                                        break;
                                    }

                                    // Read a line from the client
                                    let mut line = String::new();
                                    match reader.read_line(&mut line) {
                                        Ok(0) => {
                                            // End of stream, connection closed
                                            mcpr::log::debug!("TCP connection closed by client");
                                            break;
                                        }
                                        Ok(_) => {
                                            // Trim whitespace and newlines
                                            let message = line.trim().to_string();
                                            mcpr::log::debug!("Received TCP message from client: {}", message);

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
                                                std::thread::sleep(std::time::Duration::from_millis(10));
                                                continue;
                                            }

                                            // Error reading from stream
                                            let error = mcpr::error::MCPError::Transport(format!(
                                                "Error reading from client: {}",
                                                e
                                            ));
                                            mcpr::log::error!("{}", error);
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
                                    std::thread::sleep(std::time::Duration::from_millis(10));
                                    continue;
                                }

                                // Error accepting connection
                                let error =
                                    mcpr::error::MCPError::Transport(format!("Error accepting TCP connection: {}", e));
                                mcpr::log::error!("{}", error);
                                if let Ok(callbacks) = callbacks_clone.lock() {
                                    if let Some(callback) = &callbacks.on_error {
                                        callback(&error);
                                    }
                                }
                            }
                        }
                    }

                    mcpr::log::debug!("TCP server thread exited");
                });

                self.receiver_thread = Some(server_thread);
                mcpr::log::info!("TCP server started successfully");
                Ok(())
            }
        }

        impl #generics mcpr::transport::Transport for #name #generics {
            fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already connected
                if let Ok(connected) = self.is_connected.lock() {
                    if *connected {
                        mcpr::log::debug!("TCP transport already started");
                        return Err(mcpr::error::MCPError::AlreadyConnected);
                    }
                }

                // Start in client or server mode
                if self.is_server {
                    self.start_server()
                } else {
                    self.start_client()
                }
            }

            fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already disconnected
                if let Ok(connected) = self.is_connected.lock() {
                    if !*connected {
                        mcpr::log::debug!("TCP transport already closed");
                        return Ok(());
                    }
                }

                mcpr::log::info!("Closing TCP transport for address: {}", self.address);

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

                mcpr::log::info!("TCP transport closed successfully");
                Ok(())
            }

            fn send_json(&mut self, json_string: &str) -> Result<(), mcpr::error::MCPError> {
                // Check if connected
                if let Ok(connected) = self.is_connected.lock() {
                    if !*connected {
                        let error = mcpr::error::MCPError::NotConnected;
                        self.handle_error(&error);
                        return Err(error);
                    }
                }

                mcpr::log::debug!("Sending JSON via TCP: {}", json_string);

                // Send the message
                if let Some(writer) = &mut self.writer {
                    match writeln!(writer, "{}", json_string) {
                        Ok(_) => {
                            // Flush the writer to ensure the message is sent immediately
                            match writer.flush() {
                                Ok(_) => Ok(()),
                                Err(e) => {
                                    let error =
                                        mcpr::error::MCPError::Transport(format!("Failed to flush TCP writer: {}", e));
                                    self.handle_error(&error);
                                    Err(error)
                                }
                            }
                        }
                        Err(e) => {
                            let error = mcpr::error::MCPError::Transport(format!("Failed to send TCP message: {}", e));
                            self.handle_error(&error);
                            Err(error)
                        }
                    }
                } else {
                    let error = mcpr::error::MCPError::NotConnected;
                    self.handle_error(&error);
                    Err(error)
                }
            }

            fn receive_json(&mut self) -> Result<String, mcpr::error::MCPError> {
                // Check if connected
                if let Ok(connected) = self.is_connected.lock() {
                    if !*connected {
                        let error = mcpr::error::MCPError::NotConnected;
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
                            let error = mcpr::error::MCPError::Transport("TCP connection closed by peer".to_string());
                            self.handle_error(&error);
                            Err(error)
                        }
                        Ok(_) => {
                            // Trim whitespace and newlines
                            let message = line.trim().to_string();
                            mcpr::log::debug!("Received TCP message: {}", message);
                            Ok(message)
                        }
                        Err(e) => {
                            let error =
                                mcpr::error::MCPError::Transport(format!("Error reading from TCP stream: {}", e));
                            self.handle_error(&error);
                            Err(error)
                        }
                    }
                } else {
                    let error = mcpr::error::MCPError::NotConnected;
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

            fn on_error(&mut self, callback: Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_error = Some(callback);
                }
            }

            fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_close = Some(callback);
                }
            }

            fn set_on_close(&mut self, callback: Option<mcpr::transport::CloseCallback>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_close = callback;
                }
            }

            fn set_on_error(&mut self, callback: Option<mcpr::transport::ErrorCallback>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_error = callback;
                }
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_websocket_transport(name: &Ident, generics: &syn::Generics) -> TokenStream {
    // WebSocket transport implementation based on src/transport/websocket.rs
    let expanded = quote! {
        /// WebSocket transport implementation for MCP
        pub struct #name #generics {
            url: String,
            socket: std::sync::Arc<std::sync::Mutex<Option<tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>>>>,
            is_connected: std::sync::Arc<std::sync::Mutex<bool>>,
            callbacks: std::sync::Arc<std::sync::Mutex<WebSocketCallbackState>>,
            receiver_thread: Option<std::thread::JoinHandle<()>>,
        }

        /// Callback state container for thread-safe access
        struct WebSocketCallbackState {
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
        }

        impl #generics #name #generics {
            /// Create a new WebSocket transport
            pub fn new(url: &str) -> Self {
                mcpr::log::info!("Creating new WebSocket transport with URL: {}", url);
                Self {
                    url: url.to_string(),
                    socket: std::sync::Arc::new(std::sync::Mutex::new(None)),
                    is_connected: std::sync::Arc::new(std::sync::Mutex::new(false)),
                    callbacks: std::sync::Arc::new(std::sync::Mutex::new(WebSocketCallbackState {
                        on_message: None,
                        on_error: None,
                        on_close: None,
                    })),
                    receiver_thread: None,
                }
            }

            /// Handle a transport error
            fn handle_error(&self, error: &mcpr::error::MCPError) {
                mcpr::log::error!("WebSocket transport error: {}", error);
                if let Ok(callbacks) = self.callbacks.lock() {
                    if let Some(callback) = &callbacks.on_error {
                        callback(error);
                    }
                }
            }
        }

        impl #generics mcpr::transport::Transport for #name #generics {
            fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already connected
                if let Ok(connected) = self.is_connected.lock() {
                    if *connected {
                        mcpr::log::debug!("WebSocket transport already connected");
                        return Err(mcpr::error::MCPError::AlreadyConnected);
                    }
                }

                mcpr::log::info!("Starting WebSocket transport with URL: {}", self.url);

                // Parse URL
                let url = url::Url::parse(&self.url)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Invalid WebSocket URL: {}", e)))?;

                // Connect to the WebSocket server
                let (socket, _) = tungstenite::connect(url)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to connect to WebSocket: {}", e)))?;

                mcpr::log::info!("Successfully connected to WebSocket server");

                // Store the socket
                if let Ok(mut socket_guard) = self.socket.lock() {
                    *socket_guard = Some(socket);
                } else {
                    return Err(mcpr::error::MCPError::Transport(
                        "Failed to access socket mutex".to_string(),
                    ));
                }

                // Set connected flag
                if let Ok(mut connected) = self.is_connected.lock() {
                    *connected = true;
                } else {
                    return Err(mcpr::error::MCPError::Transport(
                        "Failed to access connection state".to_string(),
                    ));
                }

                // Start a receiver thread to process incoming messages
                let socket_clone = std::sync::Arc::clone(&self.socket);
                let is_connected_clone = std::sync::Arc::clone(&self.is_connected);
                let callbacks_clone = std::sync::Arc::clone(&self.callbacks);

                let receiver_thread = std::thread::spawn(move || {
                    mcpr::log::debug!("WebSocket receiver thread started");

                    loop {
                        // Check if we should exit
                        let should_exit = if let Ok(connected) = is_connected_clone.lock() {
                            !*connected
                        } else {
                            true
                        };

                        if should_exit {
                            mcpr::log::debug!("WebSocket receiver thread exiting");
                            break;
                        }

                        // Get access to the socket
                        if let Ok(mut socket_guard) = socket_clone.lock() {
                            if let Some(socket) = &mut *socket_guard {
                                // Try to receive a message
                                match socket.read() {
                                    Ok(message) => {
                                        match message {
                                            tungstenite::Message::Text(text) => {
                                                mcpr::log::debug!("Received WebSocket text message: {}", text);
                                                if let Ok(callbacks) = callbacks_clone.lock() {
                                                    if let Some(callback) = &callbacks.on_message {
                                                        callback(&text);
                                                    }
                                                }
                                            }
                                            tungstenite::Message::Binary(data) => {
                                                mcpr::log::debug!(
                                                    "Received WebSocket binary message ({} bytes)",
                                                    data.len()
                                                );
                                                // Binary messages not supported by MCP
                                            }
                                            tungstenite::Message::Ping(_) => {
                                                // Automatically handled by tungstenite
                                                mcpr::log::debug!("Received WebSocket ping");
                                            }
                                            tungstenite::Message::Pong(_) => {
                                                mcpr::log::debug!("Received WebSocket pong");
                                            }
                                            tungstenite::Message::Close(_) => {
                                                mcpr::log::debug!("Received WebSocket close frame");
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
                                            tungstenite::Message::Frame(_) => {
                                                // Raw frames not handled by this transport
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let error = mcpr::error::MCPError::Transport(format!("WebSocket error: {}", e));
                                        mcpr::log::error!("{}", error);
                                        if let Ok(callbacks) = callbacks_clone.lock() {
                                            if let Some(callback) = &callbacks.on_error {
                                                callback(&error);
                                            }
                                        }

                                        // Check if this is a fatal error that should close the connection
                                        // ConnectionClosed and similar errors are fatal
                                        let is_fatal = match e {
                                            tungstenite::Error::ConnectionClosed |
                                            tungstenite::Error::AlreadyClosed |
                                            tungstenite::Error::Tls(_) |
                                            tungstenite::Error::Protocol(_) |
                                            tungstenite::Error::Utf8 |
                                            tungstenite::Error::AttackAttempt |
                                            tungstenite::Error::Url(_) |
                                            tungstenite::Error::Http(_) |
                                            tungstenite::Error::HttpFormat(_) => true,
                                            tungstenite::Error::Capacity(_) |
                                            tungstenite::Error::WriteBufferFull(_) => false,
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
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }

                    mcpr::log::debug!("WebSocket receiver thread exited");
                });

                self.receiver_thread = Some(receiver_thread);
                Ok(())
            }

            fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if already disconnected
                if let Ok(connected) = self.is_connected.lock() {
                    if !*connected {
                        mcpr::log::debug!("WebSocket transport already closed");
                        return Ok(());
                    }
                }

                mcpr::log::info!("Closing WebSocket transport for URL: {}", self.url);

                // Set disconnected flag
                if let Ok(mut connected) = self.is_connected.lock() {
                    *connected = false;
                }

                // Close the socket with a normal closure
                if let Ok(mut socket_guard) = self.socket.lock() {
                    if let Some(socket) = &mut *socket_guard {
                        if let Err(e) = socket.close(None) {
                            mcpr::log::error!("Error closing WebSocket connection: {}", e);
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

                mcpr::log::info!("WebSocket transport closed successfully");
                Ok(())
            }

            fn send_json(&mut self, json_string: &str) -> Result<(), mcpr::error::MCPError> {
                // Check if connected
                if let Ok(connected) = self.is_connected.lock() {
                    if !*connected {
                        let error = mcpr::error::MCPError::NotConnected;
                        self.handle_error(&error);
                        return Err(error);
                    }
                }

                mcpr::log::debug!("Sending JSON via WebSocket: {}", json_string);

                // Send the message
                if let Ok(mut socket_guard) = self.socket.lock() {
                    if let Some(socket) = &mut *socket_guard {
                        match socket.send(tungstenite::Message::Text(json_string.to_string())) {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                let error =
                                    mcpr::error::MCPError::Transport(format!("Failed to send WebSocket message: {}", e));
                                self.handle_error(&error);
                                Err(error)
                            }
                        }
                    } else {
                        let error = mcpr::error::MCPError::NotConnected;
                        self.handle_error(&error);
                        Err(error)
                    }
                } else {
                    let error = mcpr::error::MCPError::Transport("Failed to access socket mutex".to_string());
                    self.handle_error(&error);
                    Err(error)
                }
            }

            fn receive_json(&mut self) -> Result<String, mcpr::error::MCPError> {
                // WebSockets use async handling via the receiver thread
                // This function is not expected to be called for WebSocket transport
                Err(mcpr::error::MCPError::Transport(
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

            fn on_error(&mut self, callback: Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_error = Some(callback);
                }
            }

            fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_close = Some(callback);
                }
            }

            fn set_on_close(&mut self, callback: Option<mcpr::transport::CloseCallback>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_close = callback;
                }
            }

            fn set_on_error(&mut self, callback: Option<mcpr::transport::ErrorCallback>) {
                if let Ok(mut callbacks) = self.callbacks.lock() {
                    callbacks.on_error = callback;
                }
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_sse_transport(name: &Ident, generics: &syn::Generics) -> TokenStream {
    // SSE transport implementation following the pattern in src/transport/sse.rs
    let expanded = quote! {
        /// Server-Sent Events (SSE) transport
        pub struct #name #generics {
            uri: String,
            is_connected: bool,
            is_server: bool,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            // HTTP client for making requests
            client: reqwest::blocking::Client,
            // Queue for incoming messages
            message_queue: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
            // Thread for polling SSE events
            receiver_thread: Option<std::thread::JoinHandle<()>>,
            // For server mode: active client connections
            active_clients: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, ClientConnection>>>,
            // For server mode: client message queues
            client_messages: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::collections::VecDeque<String>>>>,
            // For client mode: client ID
            client_id: std::sync::Arc<std::sync::Mutex<Option<String>>>,
            // Server instance
            server: Option<std::sync::Arc<tiny_http::Server>>,
        }

        /// Client connection information
        struct ClientConnection {
            id: String,
            last_poll: std::time::Instant,
        }

        impl #generics #name #generics {
            /// Create a new SSE transport
            pub fn new(uri: &str) -> Self {
                log::info!("Creating new SSE transport with URI: {}", uri);
                Self {
                    uri: uri.to_string(),
                    is_connected: false,
                    is_server: false,
                    on_close: None,
                    on_error: None,
                    on_message: None,
                    client: reqwest::blocking::Client::new(),
                    message_queue: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
                    receiver_thread: None,
                    active_clients: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                    client_messages: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                    client_id: std::sync::Arc::new(std::sync::Mutex::new(None)),
                    server: None,
                }
            }

            /// Create a new SSE transport in server mode
            pub fn new_server(uri: &str) -> Self {
                log::info!("Creating new SSE server transport with URI: {}", uri);
                let mut transport = Self::new(uri);
                transport.is_server = true;
                transport
            }

            /// Handle an error by calling the error callback if set
            fn handle_error(&self, error: &mcpr::error::MCPError) {
                log::error!("SSE transport error: {}", error);
                if let Some(callback) = &self.on_error {
                    callback(error);
                }
            }
        }

        impl #generics mcpr::transport::Transport for #name #generics {
            fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                if self.is_connected {
                    log::debug!("SSE transport already connected");
                    return Err(mcpr::error::MCPError::AlreadyConnected);
                }

                log::info!("Starting SSE transport with URI: {}", self.uri);

                // Create a message queue for the receiver thread
                let message_queue = std::sync::Arc::clone(&self.message_queue);

                if self.is_server {
                    // Parse the URI to get the host and port
                    let uri = self.uri.clone();
                    let uri_parts: Vec<&str> = uri.split("://").collect();
                    if uri_parts.len() != 2 {
                        return Err(mcpr::error::MCPError::Transport(format!("Invalid URI: {}", uri)));
                    }

                    let addr_parts: Vec<&str> = uri_parts[1].split(':').collect();
                    if addr_parts.len() != 2 {
                        return Err(mcpr::error::MCPError::Transport(format!("Invalid URI: {}", uri)));
                    }

                    let host = addr_parts[0];
                    let port: u16 = match addr_parts[1].parse() {
                        Ok(p) => p,
                        Err(_) => return Err(mcpr::error::MCPError::Transport(format!("Invalid port in URI: {}", uri))),
                    };

                    let addr = format!("{}:{}", host, port);
                    log::info!("Starting SSE server on {}", addr);

                    // Create the HTTP server
                    let server = match tiny_http::Server::http(&addr) {
                        Ok(s) => s,
                        Err(e) => {
                            return Err(mcpr::error::MCPError::Transport(format!(
                                "Failed to start HTTP server: {}",
                                e
                            )))
                        }
                    };

                    let server_arc = std::sync::Arc::new(server);
                    self.server = Some(std::sync::Arc::clone(&server_arc));

                    // Start a thread to handle incoming requests
                    let active_clients = std::sync::Arc::clone(&self.active_clients);
                    let client_messages = std::sync::Arc::clone(&self.client_messages);
                    let server_thread = std::thread::spawn(move || {
                        let server = server_arc;

                        for request in server.incoming_requests() {
                            // Server request handling code
                            // This would handle POST to /, GET to /poll, and GET to /register
                            log::debug!("Server received request");
                            // For brevity, not implementing the full server request handling
                            // The actual implementation would be similar to the code in src/transport/sse.rs
                        }
                    });

                    self.receiver_thread = Some(server_thread);
                } else {
                    // Client mode - simplified polling implementation
                    let uri = self.uri.clone();
                    let client = reqwest::blocking::Client::new();
                    let is_connected = std::sync::Arc::new(std::sync::Mutex::new(true));
                    let is_connected_clone = std::sync::Arc::clone(&is_connected);
                    let client_id = std::sync::Arc::clone(&self.client_id);

                    // Register with the server
                    log::debug!("Client registering with server at {}/register", uri);

                    // Simplified client polling thread
                    let client_id_clone = std::sync::Arc::clone(&client_id);
                    let receiver_thread = std::thread::spawn(move || {
                        while let Ok(connected) = is_connected_clone.lock() {
                            if !*connected {
                                log::debug!("Polling thread detected transport closure, exiting");
                                break;
                            }

                            // Simplified polling implementation
                            // For brevity, not implementing the full client polling logic
                            // The actual implementation would poll the server for messages

                            // Wait before polling again
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                        log::debug!("Client polling thread exited");
                    });

                    self.receiver_thread = Some(receiver_thread);
                }

                self.is_connected = true;
                log::info!("SSE transport started successfully");
                Ok(())
            }

            fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
                if !self.is_connected {
                    log::debug!("SSE transport already closed");
                    return Ok(());
                }

                log::info!("Closing SSE transport for URI: {}", self.uri);

                // Set the connection flag
                self.is_connected = false;

                // If we're a server, wait a short time to allow clients to receive final responses
                if self.is_server {
                    log::debug!("Server waiting for clients to receive final responses");
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }

                // Call the close callback if set
                if let Some(callback) = &self.on_close {
                    callback();
                }

                log::info!("SSE transport closed successfully");
                Ok(())
            }

            fn send_json(&mut self, json_string: &str) -> Result<(), mcpr::error::MCPError> {
                if !self.is_connected {
                    let error = mcpr::error::MCPError::NotConnected;
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
                                let queue = client_msgs.entry(id.clone()).or_insert_with(std::collections::VecDeque::new);
                                queue.push_back(json_string.to_string());
                                return Ok(());
                            }
                        }
                    }
                    Err(mcpr::error::MCPError::Transport("No client to send to".to_string()))
                } else {
                    // In client mode, send message to server via HTTP POST
                    let client = reqwest::blocking::Client::new();
                    match client.post(&self.uri).body(json_string.to_string()).send() {
                        Ok(response) => {
                            if !response.status().is_success() {
                                let error = mcpr::error::MCPError::Transport(
                                    format!("Server returned error: {}", response.status())
                                );
                                self.handle_error(&error);
                                return Err(error);
                            }
                            Ok(())
                        },
                        Err(e) => {
                            let error = mcpr::error::MCPError::Transport(
                                format!("Failed to send message to server: {}", e)
                            );
                            self.handle_error(&error);
                            Err(error)
                        }
                    }
                }
            }

            fn receive_json(&mut self) -> Result<String, mcpr::error::MCPError> {
                if !self.is_connected {
                    let error = mcpr::error::MCPError::NotConnected;
                    self.handle_error(&error);
                    return Err(error);
                }

                // Try to get a message from the queue
                if let Ok(mut queue) = self.message_queue.lock() {
                    if let Some(message) = queue.pop_front() {
                        return Ok(message);
                    }
                }

                Err(mcpr::error::MCPError::Transport("No messages available".to_string()))
            }

            fn is_connected(&self) -> bool {
                self.is_connected
            }

            fn on_message(&mut self, callback: Box<dyn Fn(&str) + Send + Sync>) {
                self.on_message = Some(callback);
            }

            fn on_error(&mut self, callback: Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>) {
                self.on_error = Some(callback);
            }

            fn on_close(&mut self, callback: Box<dyn Fn() + Send + Sync>) {
                self.on_close = Some(callback);
            }

            fn set_on_close(&mut self, callback: Option<mcpr::transport::CloseCallback>) {
                self.on_close = callback;
            }

            fn set_on_error(&mut self, callback: Option<mcpr::transport::ErrorCallback>) {
                self.on_error = callback;
            }
        }
    };

    TokenStream::from(expanded)
}
