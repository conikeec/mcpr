use super::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use crate::error::MCPError;
use serde::{de::DeserializeOwned, Serialize};
use std::io;
use std::io::{BufRead, BufReader, Write};

/// Standard IO transport
pub struct StdioTransport {
    reader: BufReader<Box<dyn io::Read + Send>>,
    writer: Box<dyn io::Write + Send>,
    is_connected: bool,
    on_close: Option<CloseCallback>,
    on_error: Option<ErrorCallback>,
    on_message: Option<MessageCallback>,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    /// Create a new stdio transport using stdin and stdout
    pub fn new() -> Self {
        Self {
            reader: io::BufReader::new(Box::new(io::stdin())),
            writer: Box::new(io::stdout()),
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }

    /// Create a new stdio transport with custom reader and writer
    pub fn with_reader_writer(
        reader: Box<dyn io::Read + Send>,
        writer: Box<dyn io::Write + Send>,
    ) -> Self {
        Self {
            reader: io::BufReader::new(reader),
            writer,
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }

    /// Handle an error by calling the error callback if set
    fn handle_error(&self, error: &MCPError) {
        if let Some(callback) = &self.on_error {
            callback(error);
        }
    }
}

impl Transport for StdioTransport {
    fn start(&mut self) -> Result<(), MCPError> {
        if self.is_connected {
            return Ok(());
        }

        self.is_connected = true;
        Ok(())
    }



    fn close(&mut self) -> Result<(), MCPError> {
        if !self.is_connected {
            return Ok(());
        }

        self.is_connected = false;

        if let Some(callback) = &self.on_close {
            callback();
        }

        Ok(())
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

    fn set_on_close(&mut self, callback: Option<CloseCallback>) {
        self.on_close = callback;
    }

    fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
        self.on_error = callback;
    }

    fn send_json(&mut self, json_string: &str) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            return Err(error);
        }

        match writeln!(self.writer, "{}", json_string) {
            Ok(_) => match self.writer.flush() {
                Ok(_) => Ok(()),
                Err(e) => {
                    let error = MCPError::Transport(format!("Failed to flush: {}", e));
                    self.handle_error(&error);
                    Err(error)
                }
            },
            Err(e) => {
                let error = MCPError::Transport(format!("Failed to write: {}", e));
                self.handle_error(&error);
                Err(error)
            }
        }
    }

    fn receive_json(&mut self) -> Result<String, MCPError> {
        if !self.is_connected {
            let error = MCPError::NotConnected;
            self.handle_error(&error);
            return Err(error);
        }

        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(_) => {
                if let Some(callback) = &self.on_message {
                    callback(&line);
                }
                Ok(line)
            }
            Err(e) => {
                let error = MCPError::Transport(format!("Failed to read: {}", e));
                self.handle_error(&error);
                Err(error)
            }
        }
    }
}
