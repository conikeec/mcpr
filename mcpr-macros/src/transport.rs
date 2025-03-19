use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, LitStr};

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
    // TCP transport implementation
    let expanded = quote! {
        pub struct #name #generics {
            stream: Option<std::net::TcpStream>,
            reader: Option<std::io::BufReader<std::net::TcpStream>>,
            writer: Option<std::io::BufWriter<std::net::TcpStream>>,
            is_connected: std::sync::atomic::AtomicBool,
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
        }

        impl #generics #name #generics {
            pub fn new() -> Self {
                Self {
                    stream: None,
                    reader: None,
                    writer: None,
                    is_connected: std::sync::atomic::AtomicBool::new(false),
                    on_message: None,
                    on_error: None,
                    on_close: None,
                }
            }

            pub fn connect(host: &str, port: u16) -> Result<Self, mcpr::error::MCPError> {
                let address = format!("{}:{}", host, port);
                let stream = std::net::TcpStream::connect(&address)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to connect to {}: {}", address, e)))?;

                // Create a clone of the stream for the reader
                let reader_stream = stream.try_clone()
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to clone TCP stream: {}", e)))?;

                Ok(Self {
                    stream: Some(stream.try_clone().unwrap()),
                    reader: Some(std::io::BufReader::new(reader_stream)),
                    writer: Some(std::io::BufWriter::new(stream)),
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
                    return Err(mcpr::error::MCPError::AlreadyConnected);
                }

                // Check if we have a stream
                if self.stream.is_none() || self.reader.is_none() || self.writer.is_none() {
                    return Err(mcpr::error::MCPError::Transport("TCP stream not initialized".to_string()));
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

                // Write message to output with newline delimiter
                writeln!(writer, "{}", json_string)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to write message: {}", e)))?;

                // Flush writer
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
                reader.read_line(&mut line)
                    .map_err(|e| mcpr::error::MCPError::Transport(format!("Failed to read message: {}", e)))?;

                // Trim whitespace
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

fn generate_websocket_transport(name: &Ident, generics: &syn::Generics) -> TokenStream {
    // WebSocket transport - stub implementation
    let expanded = quote! {
        pub struct #name #generics {
            // WebSocket implementation would go here
            is_connected: std::sync::atomic::AtomicBool,
            on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
            on_error: Option<Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>>,
            on_close: Option<Box<dyn Fn() + Send + Sync>>,
        }

        impl #generics #name #generics {
            pub fn new() -> Self {
                Self {
                    is_connected: std::sync::atomic::AtomicBool::new(false),
                    on_message: None,
                    on_error: None,
                    on_close: None,
                }
            }
        }

        impl #generics mcpr::transport::Transport for #name #generics {
            fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                Err(mcpr::error::MCPError::Transport("WebSocket transport not yet implemented".to_string()))
            }

            fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
                Err(mcpr::error::MCPError::Transport("WebSocket transport not yet implemented".to_string()))
            }

            fn send_json(&mut self, _json_string: &str) -> Result<(), mcpr::error::MCPError> {
                Err(mcpr::error::MCPError::Transport("WebSocket transport not yet implemented".to_string()))
            }

            fn receive_json(&mut self) -> Result<String, mcpr::error::MCPError> {
                Err(mcpr::error::MCPError::Transport("WebSocket transport not yet implemented".to_string()))
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
