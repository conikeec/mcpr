use crate::error::MCPError;
use crate::transport::{CloseCallback, ErrorCallback, MessageCallback, Transport};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// Standard IO transport
pub struct StdioTransport {
    reader: BufReader<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>>,
    writer_tx: mpsc::Sender<String>,
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
        // Create a channel for synchronized writing
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(32);

        // Spawn a dedicated writer task that processes one message at a time
        tokio::spawn(async move {
            let mut writer = tokio::io::BufWriter::new(tokio::io::stdout());
            while let Some(message) = writer_rx.recv().await {
                if let Err(e) = writer.write_all(message.as_bytes()).await {
                    eprintln!("Error writing to stdout: {}", e);
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    eprintln!("Error writing newline to stdout: {}", e);
                }
                if let Err(e) = writer.flush().await {
                    eprintln!("Error flushing stdout: {}", e);
                }
            }
        });

        Self {
            reader: BufReader::new(Box::new(tokio::io::stdin())),
            writer_tx,
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
        }
    }

    /// Create a new stdio transport with custom reader and writer
    pub fn with_reader(reader: Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>) -> Self {
        let mut transport = Self::new();
        transport.reader = BufReader::new(reader);
        transport
    }

    /// Handle an error by calling the error callback if set
    fn handle_error(&self, error: &MCPError) {
        if let Some(callback) = &self.on_error {
            callback(error);
        }
    }
}

// Implement Clone for StdioTransport
impl Clone for StdioTransport {
    fn clone(&self) -> Self {
        // Create a new instance with its own reader but sharing the same writer channel
        Self {
            reader: BufReader::new(Box::new(tokio::io::stdin())),
            writer_tx: self.writer_tx.clone(),
            is_connected: self.is_connected,
            on_close: None, // Callbacks cannot be cloned, create new ones when needed
            on_error: None,
            on_message: None,
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn start(&mut self) -> Result<(), MCPError> {
        if self.is_connected {
            return Ok(());
        }

        self.is_connected = true;
        Ok(())
    }

    async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        let json = match serde_json::to_string(message) {
            Ok(json) => json,
            Err(e) => {
                let error = MCPError::Serialization(e.to_string());
                self.handle_error(&error);
                return Err(error);
            }
        };

        // Send via channel to the dedicated writer task
        match self.writer_tx.send(json).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let error = MCPError::Transport(format!("Failed to send message to writer: {}", e));
                self.handle_error(&error);
                Err(error)
            }
        }
    }

    async fn receive<T: DeserializeOwned + Send + Sync>(&mut self) -> Result<T, MCPError> {
        if !self.is_connected {
            let error = MCPError::Transport("Transport not connected".to_string());
            self.handle_error(&error);
            return Err(error);
        }

        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(_) => {
                if let Some(callback) = &self.on_message {
                    callback(&line);
                }

                match serde_json::from_str(&line) {
                    Ok(parsed) => Ok(parsed),
                    Err(e) => {
                        let error = MCPError::Serialization(e.to_string());
                        self.handle_error(&error);
                        Err(error)
                    }
                }
            }
            Err(e) => {
                let error = MCPError::Transport(format!("Failed to read: {}", e));
                self.handle_error(&error);
                Err(error)
            }
        }
    }

    async fn close(&mut self) -> Result<(), MCPError> {
        if !self.is_connected {
            return Ok(());
        }

        self.is_connected = false;

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
        self.on_message = callback.map(|f| Box::new(f) as Box<dyn Fn(&str) + Send + Sync>);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    use tokio::sync::Mutex as TokioMutex;

    // Simple implementation of AsyncRead for testing
    struct MockAsyncRead {
        data: Arc<TokioMutex<Vec<String>>>,
    }

    impl MockAsyncRead {
        fn new(data: Vec<String>) -> Self {
            Self {
                data: Arc::new(TokioMutex::new(data)),
            }
        }
    }

    impl AsyncRead for MockAsyncRead {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let this = self.get_mut();

            // Use poll_fn to convert TokioMutex::lock() to poll-based API
            let mut future = Box::pin(async {
                let mut data = this.data.lock().await;
                if data.is_empty() {
                    return Ok::<(), std::io::Error>(());
                }

                let next_line = data.remove(0);
                let bytes = next_line.as_bytes();
                buf.put_slice(bytes);
                Ok::<(), std::io::Error>(())
            });

            // Convert the future to a poll
            match futures::future::FutureExt::poll_unpin(&mut future, cx) {
                Poll::Ready(result) => {
                    result?;
                    Poll::Ready(Ok(()))
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }

    // Simple implementation of AsyncWrite for testing
    struct MockAsyncWrite {
        written: Arc<TokioMutex<Vec<String>>>,
    }

    impl AsyncWrite for MockAsyncWrite {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let this = self.get_mut();

            // Use poll_fn to convert TokioMutex::lock() to poll-based API
            let mut future = Box::pin(async {
                let mut written = this.written.lock().await;
                let s = String::from_utf8_lossy(buf).to_string();
                written.push(s);
                Ok(buf.len())
            });

            // Convert the future to a poll
            futures::future::FutureExt::poll_unpin(&mut future, cx)
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    // Basic test for StdioTransport
    #[tokio::test]
    async fn test_send_receive() {
        // Test message
        let test_message = r#"{"id":1,"jsonrpc":"2.0","method":"test","params":{}}"#;

        // Create a mock reader with test data
        let mock_reader = MockAsyncRead::new(vec![test_message.to_string() + "\n"]);

        // Create a transport with the mock reader
        let mut transport = StdioTransport::with_reader(Box::new(mock_reader));

        // Start the transport
        transport.start().await.unwrap();

        // Define a simple struct to deserialize into
        #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
        struct TestMessage {
            id: u32,
            jsonrpc: String,
            method: String,
            params: serde_json::Value,
        }

        // Receive the message
        let message: TestMessage = transport.receive().await.unwrap();

        // Verify the message
        assert_eq!(message.id, 1);
        assert_eq!(message.jsonrpc, "2.0");
        assert_eq!(message.method, "test");

        // Send a response
        let response = TestMessage {
            id: 1,
            jsonrpc: "2.0".to_string(),
            method: "response".to_string(),
            params: serde_json::json!({}),
        };

        transport.send(&response).await.unwrap();

        // Close the transport
        transport.close().await.unwrap();

        // Verify the transport is closed
        assert!(!transport.is_connected);
    }

    // Test error handling for StdioTransport
    #[tokio::test]
    async fn test_error_handling() {
        // Create an empty mock reader to simulate EOF
        let mock_reader = MockAsyncRead::new(vec![]);

        // Create a transport with the mock reader
        let mut transport = StdioTransport::with_reader(Box::new(mock_reader));

        // Start the transport
        transport.start().await.unwrap();

        // Define a simple struct to deserialize into
        #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
        struct TestMessage {
            id: u32,
            jsonrpc: String,
            method: String,
            params: serde_json::Value,
        }

        // Attempt to receive from an empty stream
        let result: Result<TestMessage, MCPError> = transport.receive().await;

        // Should fail with a Transport error
        assert!(result.is_err());

        // Test error callback
        let error_received = Arc::new(Mutex::new(false));
        let error_flag = error_received.clone();

        transport.set_on_error(Some(Box::new(move |_error| {
            let mut flag = error_flag.lock().unwrap();
            *flag = true;
        })));

        // Cause another error
        let _: Result<TestMessage, _> = transport.receive().await;

        // Check if error callback was triggered
        let flag = *error_received.lock().unwrap();
        assert!(flag, "Error callback was not triggered");
    }

    // Test concurrent operations with two separate transports
    #[tokio::test]
    async fn test_concurrent_operations() {
        // Test message
        let test_message1 = r#"{"id":1,"jsonrpc":"2.0","method":"test1","params":{}}"#;
        let test_message2 = r#"{"id":2,"jsonrpc":"2.0","method":"test2","params":{}}"#;

        // Create two separate transports with their own mock readers
        let mock_reader1 = MockAsyncRead::new(vec![test_message1.to_string() + "\n"]);
        let mock_reader2 = MockAsyncRead::new(vec![test_message2.to_string() + "\n"]);

        let mut transport1 = StdioTransport::with_reader(Box::new(mock_reader1));
        let mut transport2 = StdioTransport::with_reader(Box::new(mock_reader2));

        // Start both transports
        transport1.start().await.unwrap();
        transport2.start().await.unwrap();

        // Define a simple struct to deserialize into
        #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
        struct TestMessage {
            id: u32,
            jsonrpc: String,
            method: String,
            params: serde_json::Value,
        }

        // Spawn tasks to receive messages concurrently
        let handle1 = tokio::spawn(async move {
            let message: TestMessage = transport1.receive().await.unwrap();
            message
        });

        let handle2 = tokio::spawn(async move {
            let message: TestMessage = transport2.receive().await.unwrap();
            message
        });

        // Wait for both tasks to complete
        let result1 = handle1.await.unwrap();
        let result2 = handle2.await.unwrap();

        // Verify the results
        assert_eq!(result1.id, 1);
        assert_eq!(result1.method, "test1");
        assert_eq!(result2.id, 2);
        assert_eq!(result2.method, "test2");
    }
}
