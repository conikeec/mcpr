//! High-level asynchronous client implementation for MCP
//!
//! This module provides a high-level asynchronous client implementation for the Model Context Protocol (MCP).
//! The client handles the JSON-RPC communication with the server through a transport layer and
//! provides a convenient API for initializing the connection, calling tools, and shutting down.
//!
//! All transport operations are fully asynchronous using Rust's async/await syntax, which allows
//! for efficient I/O handling and support for concurrent operations.
//!
//! ## Example
//!
//! ```rust,no_run
//! use mcpr::{client::Client, transport::stdio::StdioTransport};
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mcpr::error::MCPError> {
//!     // Create a client with stdio transport
//!     let transport = StdioTransport::new();
//!     let mut client = Client::new(transport);
//!
//!     // Initialize the client
//!     client.initialize().await?;
//!
//!     // Call a tool
//!     let request = serde_json::json!({
//!         "param1": "value1",
//!         "param2": "value2"
//!     });
//!     let response: Value = client.call_tool("my_tool", &request).await?;
//!
//!     // Process the response
//!     println!("Response: {:?}", response);
//!
//!     // Shutdown the client
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Advanced Features
//!
//! The client also supports advanced features such as:
//! - Timeouts for operations
//! - Concurrent tool calls
//! - Simplified session execution

use crate::{
    constants::LATEST_PROTOCOL_VERSION,
    error::MCPError,
    schema::json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    transport::Transport,
};
use futures::future::join_all;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

/// High-level MCP client
pub struct Client<T: Transport + Send + Sync> {
    transport: T,
    next_request_id: i64,
    timeout_duration: Option<Duration>,
}

impl<T: Transport + Send + Sync> Client<T> {
    /// Create a new MCP client with the given transport
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: 1,
            timeout_duration: None,
        }
    }

    /// Set a timeout for operations
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout_duration = Some(duration);
        self
    }

    /// Check if the client is connected to the server
    pub fn is_connected(&self) -> bool {
        // For now, we don't have a direct way to check this in the Transport trait
        // This is an estimation based on our internal state
        self.next_request_id > 1 // If we've sent at least one request, we consider ourselves connected
    }

    /// Initialize the client
    pub async fn initialize(&mut self) -> Result<Value, MCPError> {
        // Start the transport
        self.transport.start().await?;

        // Send initialization request
        let initialize_request = JSONRPCRequest::new(
            self.next_request_id(),
            "initialize".to_string(),
            Some(serde_json::json!({
                "protocol_version": LATEST_PROTOCOL_VERSION
            })),
        );

        let message = JSONRPCMessage::Request(initialize_request);
        self.transport.send(&message).await?;

        // Wait for response with timeout if set
        let response: JSONRPCMessage = self.receive_with_timeout().await?;

        match response {
            JSONRPCMessage::Response(resp) => Ok(resp.result),
            JSONRPCMessage::Error(err) => Err(MCPError::Protocol(format!(
                "Initialization failed: {:?}",
                err
            ))),
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Call a tool on the server
    pub async fn call_tool<P: Serialize + Send + Sync, R: DeserializeOwned + Send + Sync>(
        &mut self,
        tool_name: &str,
        params: &P,
    ) -> Result<R, MCPError> {
        // Create tool call request
        let tool_call_request = JSONRPCRequest::new(
            self.next_request_id(),
            "tool_call".to_string(),
            Some(serde_json::json!({
                "name": tool_name,
                "parameters": serde_json::to_value(params)?
            })),
        );

        let message = JSONRPCMessage::Request(tool_call_request);
        self.transport.send(&message).await?;

        // Wait for response with timeout if set
        let response: JSONRPCMessage = self.receive_with_timeout().await?;

        match response {
            JSONRPCMessage::Response(resp) => {
                // Extract the tool result from the response
                let result_value = resp.result;
                let result = result_value.get("result").ok_or_else(|| {
                    MCPError::Protocol("Missing 'result' field in response".to_string())
                })?;

                // Parse the result
                serde_json::from_value(result.clone())
                    .map_err(|e| MCPError::Serialization(e.to_string()))
            }
            JSONRPCMessage::Error(err) => {
                Err(MCPError::Protocol(format!("Tool call failed: {:?}", err)))
            }
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Shutdown the client
    pub async fn shutdown(&mut self) -> Result<(), MCPError> {
        // Send shutdown request
        let shutdown_request =
            JSONRPCRequest::new(self.next_request_id(), "shutdown".to_string(), None);

        let message = JSONRPCMessage::Request(shutdown_request);
        self.transport.send(&message).await?;

        // Wait for response with timeout if set
        let response: JSONRPCMessage = self.receive_with_timeout().await?;

        match response {
            JSONRPCMessage::Response(_) => {
                // Close the transport
                self.transport.close().await?;
                Ok(())
            }
            JSONRPCMessage::Error(err) => {
                Err(MCPError::Protocol(format!("Shutdown failed: {:?}", err)))
            }
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Receive a message with optional timeout
    async fn receive_with_timeout<R: DeserializeOwned + Send + Sync>(
        &mut self,
    ) -> Result<R, MCPError> {
        if let Some(duration) = self.timeout_duration {
            match timeout(duration, self.transport.receive::<R>()).await {
                Ok(result) => result,
                Err(_) => Err(MCPError::Timeout(format!(
                    "Operation timed out after {:?}",
                    duration
                ))),
            }
        } else {
            self.transport.receive().await
        }
    }

    /// Generate the next request ID
    fn next_request_id(&mut self) -> RequestId {
        let id = self.next_request_id;
        self.next_request_id += 1;
        RequestId::Number(id)
    }

    /// Call multiple tools concurrently
    ///
    /// This method demonstrates the power of async by allowing multiple tool calls to be made
    /// concurrently. Each tool call is represented as a tuple of (tool_name, parameters).
    ///
    /// Note that this requires cloning the transport for each concurrent call, which may not
    /// be supported by all transport types. Use with caution.
    pub async fn call_tools_concurrent<P, R>(
        &self,
        tool_calls: Vec<(String, P)>,
    ) -> Result<Vec<Result<R, MCPError>>, MCPError>
    where
        P: Serialize + Clone + Send + Sync + 'static,
        R: DeserializeOwned + Send + Sync + 'static,
        T: Clone + Send + Sync + 'static,
    {
        let mut tasks = Vec::with_capacity(tool_calls.len());

        // Create a new client for each concurrent call
        for (idx, (tool_name, params)) in tool_calls.into_iter().enumerate() {
            let mut client = Client {
                transport: self.transport.clone(),
                next_request_id: self.next_request_id + idx as i64 + 1, // Ensure unique IDs
                timeout_duration: self.timeout_duration,
            };

            // Spawn a task for each tool call
            let task =
                tokio::spawn(async move { client.call_tool::<P, R>(&tool_name, &params).await });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let results = join_all(tasks).await;

        // Process results, handling any JoinErrors from the tasks
        let processed_results = results
            .into_iter()
            .map(|join_result| match join_result {
                Ok(tool_result) => tool_result,
                Err(e) => Err(MCPError::Transport(format!("Task join error: {}", e))),
            })
            .collect();

        Ok(processed_results)
    }

    /// Execute a complete client session in one call
    ///
    /// This method handles initialization, tool calls, and shutdown in a single method call.
    /// It's especially useful for short-lived clients that need to make a few tool calls
    /// and then shut down.
    ///
    /// # Example
    /// ```ignore
    /// use mcpr::{client::Client, transport::stdio::StdioTransport};
    /// use serde_json::json;
    ///
    /// // Create a client with stdio transport
    /// let mut client = Client::new(StdioTransport::new());
    ///
    /// // Execute a session with initialization and shutdown handled automatically
    /// let result = client.execute_session(|client| {
    ///     Box::pin(async move {
    ///         // Call a tool and return the result
    ///         client.call_tool("example_tool", &json!({"param": "value"})).await
    ///     })
    /// }).await;
    /// ```
    pub async fn execute_session<F, Fut, R>(&mut self, f: F) -> Result<R, MCPError>
    where
        F: FnOnce(&mut Self) -> Fut + Send,
        Fut: std::future::Future<Output = Result<R, MCPError>> + Send,
        R: Send,
    {
        // Initialize the client
        self.initialize().await?;

        // Execute the user-provided function
        let result = f(self).await;

        // Shutdown the client regardless of whether the function succeeded
        let shutdown_result = self.shutdown().await;

        // Return the function result if it succeeded, otherwise return the function error
        match result {
            Ok(r) => {
                // If the function succeeded but shutdown failed, return the shutdown error
                if let Err(e) = shutdown_result {
                    Err(e)
                } else {
                    Ok(r)
                }
            }
            Err(e) => {
                // If both the function and shutdown failed, prefer the function error
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::json_rpc::{JSONRPCError, JSONRPCMessage, JSONRPCResponse, RequestId};
    use crate::transport::Transport;
    use crate::transport::{CloseCallback, ErrorCallback, MessageCallback};
    use async_trait::async_trait;
    use futures::future::Future;
    use std::collections::VecDeque;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::Mutex as TokioMutex;

    // Mock transport for testing
    #[derive(Clone)]
    struct MockTransport {
        send_queue: Arc<TokioMutex<VecDeque<String>>>,
        receive_queue: Arc<TokioMutex<VecDeque<String>>>,
        is_started: Arc<TokioMutex<bool>>,
        is_closed: Arc<TokioMutex<bool>>,
        should_fail: Arc<TokioMutex<bool>>,
        simulate_timeout: Arc<TokioMutex<bool>>,
        on_message: Arc<Mutex<Option<MessageCallback>>>,
        on_error: Arc<Mutex<Option<ErrorCallback>>>,
        on_close: Arc<Mutex<Option<CloseCallback>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                send_queue: Arc::new(TokioMutex::new(VecDeque::new())),
                receive_queue: Arc::new(TokioMutex::new(VecDeque::new())),
                is_started: Arc::new(TokioMutex::new(false)),
                is_closed: Arc::new(TokioMutex::new(false)),
                should_fail: Arc::new(TokioMutex::new(false)),
                simulate_timeout: Arc::new(TokioMutex::new(false)),
                on_message: Arc::new(Mutex::new(None)),
                on_error: Arc::new(Mutex::new(None)),
                on_close: Arc::new(Mutex::new(None)),
            }
        }

        async fn queue_message(&self, message: JSONRPCMessage) {
            let serialized = serde_json::to_string(&message).unwrap();
            let mut queue = self.receive_queue.lock().await;
            queue.push_back(serialized);
        }

        async fn get_last_sent(&self) -> Option<String> {
            let mut queue = self.send_queue.lock().await;
            queue.pop_front()
        }

        async fn set_should_fail(&self, should_fail: bool) {
            let mut fail = self.should_fail.lock().await;
            *fail = should_fail;
        }

        async fn set_simulate_timeout(&self, timeout: bool) {
            let mut t = self.simulate_timeout.lock().await;
            *t = timeout;
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn start(&mut self) -> Result<(), MCPError> {
            let should_fail = *self.should_fail.lock().await;
            if should_fail {
                return Err(MCPError::Transport(
                    "Mock transport failed to start".to_string(),
                ));
            }

            let mut started = self.is_started.lock().await;
            *started = true;
            Ok(())
        }

        async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError> {
            let should_fail = *self.should_fail.lock().await;
            if should_fail {
                return Err(MCPError::Transport(
                    "Mock transport send failure".to_string(),
                ));
            }

            let serialized = serde_json::to_string(message)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            let mut queue = self.send_queue.lock().await;
            queue.push_back(serialized);
            Ok(())
        }

        async fn receive<T: DeserializeOwned + Send + Sync>(&mut self) -> Result<T, MCPError> {
            let should_fail = *self.should_fail.lock().await;
            if should_fail {
                return Err(MCPError::Transport(
                    "Mock transport receive failure".to_string(),
                ));
            }

            let simulate_timeout = *self.simulate_timeout.lock().await;
            if simulate_timeout {
                // Simulate a long operation
                tokio::time::sleep(Duration::from_secs(2)).await;
            }

            let mut queue = self.receive_queue.lock().await;

            if let Some(message) = queue.pop_front() {
                // If there's a message callback, call it
                if let Some(callback) = &*self.on_message.lock().unwrap() {
                    callback(&message);
                }

                return serde_json::from_str(&message)
                    .map_err(|e| MCPError::Serialization(e.to_string()));
            }

            Err(MCPError::Transport("No more messages".to_string()))
        }

        async fn close(&mut self) -> Result<(), MCPError> {
            let should_fail = *self.should_fail.lock().await;
            if should_fail {
                return Err(MCPError::Transport(
                    "Mock transport close failure".to_string(),
                ));
            }

            let mut closed = self.is_closed.lock().await;
            *closed = true;

            // If there's a close callback, call it
            if let Some(callback) = &*self.on_close.lock().unwrap() {
                callback();
            }

            Ok(())
        }

        fn set_on_close(&mut self, callback: Option<CloseCallback>) {
            let mut cb = self.on_close.lock().unwrap();
            *cb = callback;
        }

        fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
            let mut cb = self.on_error.lock().unwrap();
            *cb = callback;
        }

        fn set_on_message<F>(&mut self, callback: Option<F>)
        where
            F: Fn(&str) + Send + Sync + 'static,
        {
            let mut cb = self.on_message.lock().unwrap();
            if let Some(callback) = callback {
                *cb = Some(Box::new(callback));
            } else {
                *cb = None;
            }
        }
    }

    // Helper function to create a server info response
    fn create_initialize_response(id: RequestId) -> JSONRPCMessage {
        JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: serde_json::json!({
                "protocol_version": "1.0",
                "server_info": {
                    "name": "TestServer",
                    "version": "1.0.0",
                    "protocol_version": "1.0"
                },
                "capabilities": {
                    "tools": true
                }
            }),
        })
    }

    // Helper function to create a tools list response
    fn create_tools_list_response(id: RequestId) -> JSONRPCMessage {
        JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: serde_json::json!([
                {
                    "name": "hello",
                    "description": "Say hello",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string"
                            }
                        }
                    }
                }
            ]),
        })
    }

    // Helper function to create a tool call response
    fn create_tool_call_response(id: RequestId, result: serde_json::Value) -> JSONRPCMessage {
        JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: serde_json::json!({
                "result": result
            }),
        })
    }

    // Basic client initialization test
    #[tokio::test]
    async fn test_client_initialization() {
        // Create a mock transport
        let mut mock = MockTransport::new();

        // Queue the initialize response
        mock.queue_message(create_initialize_response(RequestId::Number(1)))
            .await;

        // Create client with mock transport
        let mut client = Client::new(mock.clone());

        // Test initialization
        let result = client.initialize().await;
        assert!(result.is_ok(), "Client initialization should succeed");

        // Check what was sent to the server
        let sent = mock.get_last_sent().await.unwrap();
        let sent_msg: JSONRPCMessage = serde_json::from_str(&sent).unwrap();

        if let JSONRPCMessage::Request(req) = sent_msg {
            assert_eq!(req.method, "initialize");
        } else {
            panic!("Expected request message");
        }
    }

    // Test client error handling
    #[tokio::test]
    async fn test_client_error_handling() {
        // Create a mock transport that will fail
        let mut mock = MockTransport::new();
        mock.set_should_fail(true).await;

        // Create client with mock transport
        let mut client = Client::new(mock);

        // Test initialization with failing transport
        let result = client.initialize().await;
        assert!(
            result.is_err(),
            "Client initialization should fail with failing transport"
        );
    }

    // Test tool calling
    #[tokio::test]
    async fn test_tool_call() {
        // Create a mock transport
        let mut mock = MockTransport::new();

        // Queue the initialize response
        mock.queue_message(create_initialize_response(RequestId::Number(1)))
            .await;

        // Queue the tool call response
        mock.queue_message(create_tool_call_response(
            RequestId::Number(2),
            serde_json::json!("Hello, Test User!"),
        ))
        .await;

        // Create client with mock transport
        let mut client = Client::new(mock.clone());

        // Initialize the client
        client.initialize().await.unwrap();

        // Call a tool
        let params = serde_json::json!({
            "name": "Test User"
        });

        let result: String = client.call_tool("hello", &params).await.unwrap();
        assert_eq!(result, "Hello, Test User!");

        // Check what was sent to the server
        let init_msg = mock.get_last_sent().await.unwrap();
        let tool_msg = mock.get_last_sent().await.unwrap();

        let tool_req: JSONRPCMessage = serde_json::from_str(&tool_msg).unwrap();
        if let JSONRPCMessage::Request(req) = tool_req {
            assert_eq!(req.method, "tool_call");
            if let Some(params) = req.params {
                assert_eq!(params["name"], "hello");
            } else {
                panic!("Expected tool call params");
            }
        } else {
            panic!("Expected request message");
        }
    }

    // Test timeout handling
    #[tokio::test]
    async fn test_timeout_handling() {
        // Create a mock transport that will simulate a timeout
        let mut mock = MockTransport::new();
        mock.set_simulate_timeout(true).await;

        // Queue the initialize response (but it won't be used due to timeout)
        mock.queue_message(create_initialize_response(RequestId::Number(1)))
            .await;

        // Create client with mock transport and a short timeout
        let mut client = Client::new(mock).with_timeout(Duration::from_millis(100));

        // Test initialization with timeout
        let result = client.initialize().await;
        assert!(result.is_err(), "Client initialization should timeout");

        if let Err(MCPError::Timeout(_)) = result {
            // Expected timeout error
        } else {
            panic!("Expected timeout error but got: {:?}", result);
        }
    }

    // Test shutdown
    #[tokio::test]
    async fn test_shutdown() {
        // Create a mock transport
        let mut mock = MockTransport::new();

        // Queue the initialize response
        mock.queue_message(create_initialize_response(RequestId::Number(1)))
            .await;

        // Queue the shutdown response
        mock.queue_message(JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(2),
            result: serde_json::json!({}),
        }))
        .await;

        // Create client with mock transport
        let mut client = Client::new(mock.clone());

        // Initialize the client
        client.initialize().await.unwrap();

        // Test shutdown
        let result = client.shutdown().await;
        assert!(result.is_ok(), "Client shutdown should succeed");

        // Verify the transport was closed
        let is_closed = *mock.is_closed.lock().await;
        assert!(is_closed, "Transport should be closed after shutdown");
    }

    // Test error response handling
    #[tokio::test]
    async fn test_error_response_handling() {
        // Create a mock transport
        let mut mock = MockTransport::new();

        // Queue an error response for initialization
        mock.queue_message(JSONRPCMessage::Error(JSONRPCError {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(1),
            error: crate::schema::json_rpc::JSONRPCErrorObject {
                code: -32000,
                message: "Test error".to_string(),
                data: None,
            },
        }))
        .await;

        // Create client with mock transport
        let mut client = Client::new(mock.clone());

        // Test initialization with error response
        let result = client.initialize().await;
        assert!(
            result.is_err(),
            "Client initialization should fail with error response"
        );

        if let Err(MCPError::Protocol(msg)) = result {
            assert!(
                msg.contains("Test error"),
                "Error message should contain the error from the response"
            );
        } else {
            panic!("Expected protocol error but got: {:?}", result);
        }
    }

    // Test concurrent tool calls
    #[tokio::test]
    async fn test_concurrent_tool_calls() -> Result<(), MCPError> {
        // Create a mock transport
        let mut mock = MockTransport::new();

        // Queue the initialize response
        mock.queue_message(create_initialize_response(RequestId::Number(1)))
            .await;

        // Queue two tool call responses (they will be consumed in order)
        mock.queue_message(create_tool_call_response(
            RequestId::Number(2),
            serde_json::json!("Result 1"),
        ))
        .await;

        mock.queue_message(create_tool_call_response(
            RequestId::Number(3),
            serde_json::json!("Result 2"),
        ))
        .await;

        // Create client with mock transport
        let mut client = Client::new(mock.clone());

        // Initialize the client
        client.initialize().await?;

        // Create two tool calls to execute concurrently
        let tool_calls = vec![
            ("tool1".to_string(), serde_json::json!({"param": "value1"})),
            ("tool2".to_string(), serde_json::json!({"param": "value2"})),
        ];

        // Call tools concurrently
        let results: Vec<Result<String, MCPError>> =
            client.call_tools_concurrent(tool_calls).await?;

        // Verify results
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());

        assert_eq!(results[0].as_ref().unwrap(), "Result 1");
        assert_eq!(results[1].as_ref().unwrap(), "Result 2");

        // Verify the requests were sent
        let init_msg = mock.get_last_sent().await.unwrap();
        let tool1_msg = mock.get_last_sent().await.unwrap();
        let tool2_msg = mock.get_last_sent().await.unwrap();

        // Parse and verify tool call requests
        let tool1_req: JSONRPCMessage = serde_json::from_str(&tool1_msg).unwrap();
        let tool2_req: JSONRPCMessage = serde_json::from_str(&tool2_msg).unwrap();

        if let JSONRPCMessage::Request(req) = tool1_req {
            assert_eq!(req.method, "tool_call");
            if let Some(params) = req.params {
                assert_eq!(params["name"], "tool1");
                assert_eq!(params["parameters"]["param"], "value1");
            }
        }

        if let JSONRPCMessage::Request(req) = tool2_req {
            assert_eq!(req.method, "tool_call");
            if let Some(params) = req.params {
                assert_eq!(params["name"], "tool2");
                assert_eq!(params["parameters"]["param"], "value2");
            }
        }

        Ok(())
    }
}
