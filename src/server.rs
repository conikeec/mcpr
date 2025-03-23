//! High-level asynchronous server implementation for MCP
//!
//! This module provides a high-level asynchronous server implementation for the Model Context Protocol (MCP).
//! The server handles the JSON-RPC communication with clients through a transport layer and
//! provides a convenient API for registering tool handlers, processing requests, and managing the connection lifecycle.
//!
//! All transport operations are fully asynchronous using Rust's async/await syntax, which allows
//! for efficient I/O handling and support for concurrent client requests.
//!
//! ## Example
//!
//! ```rust,no_run
//! use mcpr::{
//!     error::MCPError,
//!     server::{Server, ServerConfig},
//!     transport::stdio::StdioTransport,
//!     schema::common::Tool,
//! };
//! use serde_json::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), MCPError> {
//!     // Configure the server
//!     let server_config = ServerConfig::new()
//!         .with_name("My MCP Server")
//!         .with_version("1.0.0")
//!         .with_tool(Tool {
//!             name: "my_tool".to_string(),
//!             description: Some("My awesome tool".to_string()),
//!             input_schema: mcpr::schema::common::ToolInputSchema {
//!                 r#type: "object".to_string(),
//!                 properties: Some([
//!                     ("param1".to_string(), serde_json::json!({
//!                         "type": "string",
//!                         "description": "First parameter"
//!                     })),
//!                     ("param2".to_string(), serde_json::json!({
//!                         "type": "string",
//!                         "description": "Second parameter"
//!                     }))
//!                 ].into_iter().collect()),
//!                 required: Some(vec!["param1".to_string(), "param2".to_string()]),
//!             },
//!         });
//!
//!     // Create the server
//!     let mut server = Server::new(server_config);
//!
//!     // Register tool handlers
//!     server.register_tool_handler("my_tool", |params: Value| async move {
//!         // Parse parameters and handle the tool call
//!         let param1 = params.get("param1")
//!             .and_then(|v| v.as_str())
//!             .ok_or_else(|| MCPError::Protocol("Missing param1".to_string()))?;
//!
//!         let param2 = params.get("param2")
//!             .and_then(|v| v.as_str())
//!             .ok_or_else(|| MCPError::Protocol("Missing param2".to_string()))?;
//!
//!         // Process the parameters and generate a response
//!         let response = serde_json::json!({
//!             "result": format!("Processed {} and {}", param1, param2)
//!         });
//!
//!         Ok(response)
//!     })?;
//!
//!     // Start the server with stdio transport
//!     let transport = StdioTransport::new();
//!     server.serve(transport).await
//! }
//! ```

use crate::{
    constants::LATEST_PROTOCOL_VERSION,
    error::MCPError,
    schema::{
        client::{CallToolParams, ListPromptsResult, ListResourcesResult, ListToolsResult},
        common::{Implementation, Prompt, Resource, Tool},
        json_rpc::{JSONRPCMessage, JSONRPCResponse, RequestId},
        server::{
            CallToolResult, InitializeResult, PromptsCapability, ResourcesCapability,
            ServerCapabilities, ToolResultContent, ToolsCapability,
        },
    },
    transport::Transport,
};
use futures::future::join_all;
use log::{debug, error, info};
use serde_json::{json, Value};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::timeout};

/// Server configuration
#[derive(Clone)]
pub struct ServerConfig {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Available tools
    pub tools: Vec<Tool>,
    /// Available prompts
    pub prompts: Vec<Prompt>,
    /// Available resources
    pub resources: Vec<Resource>,
    /// Timeout for operations (in milliseconds)
    pub timeout: Option<Duration>,
}

impl ServerConfig {
    /// Create a new server configuration
    pub fn new() -> Self {
        Self {
            name: "MCP Server".to_string(),
            version: "1.0.0".to_string(),
            tools: Vec::new(),
            prompts: Vec::new(),
            resources: Vec::new(),
            timeout: None,
        }
    }

    /// Set the server name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set the server version
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    /// Add a tool to the server
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add a prompt to the server
    pub fn with_prompt(mut self, prompt: Prompt) -> Self {
        self.prompts.push(prompt);
        self
    }

    /// Add a resource to the server
    pub fn with_resource(mut self, resource: Resource) -> Self {
        self.resources.push(resource);
        self
    }

    /// Set a timeout for operations
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool handler function type for async tool execution
/// Returns a boxed future that resolves to a Result with the tool's result or an error
pub type AsyncToolHandler = Box<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, MCPError>> + Send>> + Send + Sync,
>;

/// High-level MCP server
#[derive(Clone)]
pub struct Server<T: Transport + Send + Sync> {
    config: ServerConfig,
    tool_handlers: Arc<Mutex<HashMap<String, AsyncToolHandler>>>,
    transport: Option<T>,
    shutdown_requested: Arc<Mutex<bool>>,
}

impl<T: Transport + Send + Sync + Clone + 'static> Server<T> {
    /// Create a new MCP server with the given configuration
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            tool_handlers: Arc::new(Mutex::new(HashMap::new())),
            transport: None,
            shutdown_requested: Arc::new(Mutex::new(false)),
        }
    }

    /// Register a tool handler
    pub fn register_tool_handler<F, Fut>(
        &mut self,
        tool_name: &str,
        handler: F,
    ) -> Result<(), MCPError>
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, MCPError>> + Send + 'static,
    {
        // Check if the tool exists in the configuration
        if !self.config.tools.iter().any(|t| t.name == tool_name) {
            return Err(MCPError::Protocol(format!(
                "Tool '{}' not found in server configuration",
                tool_name
            )));
        }

        // Create a wrapper that returns a boxed future
        let async_handler: AsyncToolHandler = Box::new(move |params| {
            let fut = handler(params);
            Box::pin(fut) as Pin<Box<dyn Future<Output = Result<Value, MCPError>> + Send>>
        });

        // Register the handler
        let mut handlers = match self.tool_handlers.try_lock() {
            Ok(handlers) => handlers,
            Err(_) => {
                return Err(MCPError::Protocol(
                    "Failed to acquire lock on tool handlers".to_string(),
                ))
            }
        };

        handlers.insert(tool_name.to_string(), async_handler);

        Ok(())
    }

    /// Start the server with the given transport
    pub async fn serve(&mut self, mut transport: T) -> Result<(), MCPError> {
        // Start the transport
        transport.start().await?;

        // Store the transport
        self.transport = Some(transport);

        // Process messages
        self.process_messages().await
    }

    /// Start the server with the given transport in the background
    /// Returns immediately without blocking the caller
    pub async fn start_background(&mut self, mut transport: T) -> Result<(), MCPError> {
        // Start the transport
        transport.start().await?;

        // Clone the server for the background task
        let mut server_clone = self.clone();

        // Store the transport in both instances
        self.transport = Some(transport.clone());
        server_clone.transport = Some(transport);

        // Spawn a background task to run the message processing loop
        tokio::spawn(async move {
            if let Err(e) = server_clone.process_messages().await {
                error!("Error in server background task: {}", e);
            }
        });

        // Return immediately
        info!("Server started in background mode");

        Ok(())
    }

    /// Process incoming messages
    async fn process_messages(&mut self) -> Result<(), MCPError> {
        loop {
            // Check if shutdown was requested
            {
                let shutdown = *self.shutdown_requested.lock().await;
                if shutdown {
                    break;
                }
            }

            let message = {
                let transport = self
                    .transport
                    .as_mut()
                    .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

                // Receive a message with timeout if configured
                if let Some(duration) = self.config.timeout {
                    match timeout(duration, transport.receive::<JSONRPCMessage>()).await {
                        Ok(result) => match result {
                            Ok(msg) => msg,
                            Err(e) => {
                                error!("Error receiving message: {}", e);
                                continue;
                            }
                        },
                        Err(_) => {
                            error!("Receive operation timed out");
                            continue;
                        }
                    }
                } else {
                    // No timeout
                    match transport.receive::<JSONRPCMessage>().await {
                        Ok(msg) => msg,
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                            continue;
                        }
                    }
                }
            };

            // Handle the message
            match message {
                JSONRPCMessage::Request(request) => {
                    let id = request.id.clone();
                    let method = request.method.clone();
                    let params = request.params.clone();

                    match method.as_str() {
                        "initialize" => {
                            info!("Received initialization request");
                            if let Err(e) = self.handle_initialize(id, params).await {
                                error!("Error handling initialize request: {}", e);
                            }
                        }
                        "tools/list" => {
                            info!("Received tools list request");
                            if let Err(e) = self.handle_tools_list(id, params).await {
                                error!("Error handling tools/list request: {}", e);
                            }
                        }
                        "tools/call" => {
                            info!("Received tools/call request");
                            // Process tools/call requests in a new task
                            let tools_call_task = self.clone_for_tools_call();
                            let id_clone = id.clone();
                            let params_clone = params.clone();

                            // Spawn a new task to handle the tool call concurrently
                            tokio::spawn(async move {
                                if let Err(e) = tools_call_task
                                    .handle_tools_call(id_clone, params_clone)
                                    .await
                                {
                                    error!("Error handling tools/call request: {}", e);
                                }
                            });
                        }
                        "tool_call" => {
                            // Legacy method for backward compatibility
                            info!("Received legacy tool_call request (redirecting to tools/call)");
                            // Process tools/call requests in a new task
                            let tools_call_task = self.clone_for_tools_call();
                            let id_clone = id.clone();
                            let params_clone = params.clone();

                            // Spawn a new task to handle the tool call concurrently
                            tokio::spawn(async move {
                                if let Err(e) = tools_call_task
                                    .handle_tools_call(id_clone, params_clone)
                                    .await
                                {
                                    error!("Error handling tool_call request: {}", e);
                                }
                            });
                        }
                        "prompts/list" => {
                            info!("Received prompts list request");
                            if let Err(e) = self.handle_prompts_list(id, params).await {
                                error!("Error handling prompts/list request: {}", e);
                            }
                        }
                        "resources/list" => {
                            info!("Received resources list request");
                            if let Err(e) = self.handle_resources_list(id, params).await {
                                error!("Error handling resources/list request: {}", e);
                            }
                        }
                        "ping" => {
                            debug!("Received ping request");
                            if let Err(e) = self.handle_ping(id).await {
                                error!("Error handling ping request: {}", e);
                            }
                        }
                        "$/cancelRequest" => {
                            debug!("Received cancel request");
                            if let Err(e) = self.handle_cancel_request(id, params).await {
                                error!("Error handling cancel request: {}", e);
                            }
                        }
                        "shutdown" => {
                            info!("Received shutdown request");
                            if let Err(e) = self.handle_shutdown(id).await {
                                error!("Error handling shutdown request: {}", e);
                            }
                            // Mark shutdown as requested
                            let mut shutdown = self.shutdown_requested.lock().await;
                            *shutdown = true;
                            break;
                        }
                        _ => {
                            error!("Unknown method: {}", method);
                            if let Err(e) = self
                                .send_error(
                                    id,
                                    -32601,
                                    format!("Method not found: {}", method),
                                    None,
                                )
                                .await
                            {
                                error!("Error sending error response: {}", e);
                            }
                        }
                    }
                }
                JSONRPCMessage::Notification(notification) => {
                    let method = notification.method.clone();

                    match method.as_str() {
                        "initialized" => {
                            info!("Received 'initialized' notification - client is ready");
                            // The initialized notification doesn't require a response
                            // Just acknowledge receipt and continue processing
                        }
                        _ => {
                            debug!("Received unknown notification: {}", method);
                        }
                    }
                }
                _ => {
                    error!("Unexpected message type");
                    continue;
                }
            }
        }

        // Close the transport if we're exiting the loop
        if let Some(transport) = self.transport.as_mut() {
            transport.close().await?;
        }

        Ok(())
    }

    /// Create a clone of the server for handling tool calls concurrently
    fn clone_for_tools_call(&self) -> ToolCallHandler<T>
    where
        T: Clone,
    {
        ToolCallHandler {
            tool_handlers: self.tool_handlers.clone(),
            transport: self.transport.as_ref().cloned(),
        }
    }

    /// Handle initialization request
    async fn handle_initialize(
        &mut self,
        id: RequestId,
        _params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create server capabilities with tool support
        let capabilities = ServerCapabilities {
            experimental: None,
            logging: None,
            prompts: if !self.config.prompts.is_empty() {
                Some(PromptsCapability {
                    list_changed: Some(false),
                })
            } else {
                None
            },
            resources: if !self.config.resources.is_empty() {
                Some(ResourcesCapability {
                    list_changed: Some(false),
                    subscribe: Some(false),
                })
            } else {
                None
            },
            tools: if !self.config.tools.is_empty() {
                Some(ToolsCapability {
                    list_changed: Some(false),
                })
            } else {
                None
            },
        };

        // Create server information
        let server_info = Implementation {
            name: self.config.name.clone(),
            version: self.config.version.clone(),
        };

        // Create initialization result
        let init_result = InitializeResult {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            server_info,
            instructions: None,
        };

        // Create response with proper result
        let response = JSONRPCResponse::new(
            id,
            serde_json::to_value(init_result)
                .map_err(|e| MCPError::Serialization(e.to_string()))?,
        );

        // Send the response
        transport.send(&JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle tools list request
    async fn handle_tools_list(
        &mut self,
        id: RequestId,
        _params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create tools list result
        let tools_list = ListToolsResult {
            next_cursor: None, // No pagination in this implementation
            tools: self.config.tools.clone(),
        };

        // Create response with proper result
        let response = JSONRPCResponse::new(
            id,
            serde_json::to_value(tools_list).map_err(|e| MCPError::Serialization(e.to_string()))?,
        );

        // Send the response
        transport.send(&JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle prompts list request
    async fn handle_prompts_list(
        &mut self,
        id: RequestId,
        _params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create prompts list result
        let prompts_list = ListPromptsResult {
            next_cursor: None, // No pagination in this implementation
            prompts: self.config.prompts.clone(),
        };

        // Create response with proper result
        let response = JSONRPCResponse::new(
            id,
            serde_json::to_value(prompts_list)
                .map_err(|e| MCPError::Serialization(e.to_string()))?,
        );

        // Send the response
        transport.send(&JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle resources list request
    async fn handle_resources_list(
        &mut self,
        id: RequestId,
        _params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create resources list result
        let resources_list = ListResourcesResult {
            next_cursor: None, // No pagination in this implementation
            resources: self.config.resources.clone(),
        };

        // Create response with proper result
        let response = JSONRPCResponse::new(
            id,
            serde_json::to_value(resources_list)
                .map_err(|e| MCPError::Serialization(e.to_string()))?,
        );

        // Send the response
        transport.send(&JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle shutdown request
    async fn handle_shutdown(&mut self, id: RequestId) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create shutdown response
        let response = JSONRPCResponse::new(id, serde_json::json!({}));

        // Send the response
        transport.send(&JSONRPCMessage::Response(response)).await?;

        Ok(())
    }

    /// Handle cancel request
    async fn handle_cancel_request(
        &mut self,
        id: RequestId,
        params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Extract the ID of the request to cancel
        if let Some(params) = params {
            if let Some(request_id) = params.get("id") {
                debug!("Request to cancel operation with ID: {:?}", request_id);

                // In a real implementation, you would use the request_id to find and cancel
                // the corresponding in-progress operation
                // For now, we'll just acknowledge the cancellation

                // Send a success response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                });

                transport.send(&response).await?;
                info!("Acknowledged cancellation request for ID: {:?}", request_id);
            } else {
                // Missing required parameter
                return self
                    .send_error(
                        id,
                        -32602,
                        "Missing required parameter 'id'".to_string(),
                        None,
                    )
                    .await;
            }
        } else {
            // Missing parameters
            return self
                .send_error(id, -32602, "Missing required parameters".to_string(), None)
                .await;
        }

        Ok(())
    }

    /// Send an error response
    async fn send_error(
        &mut self,
        id: RequestId,
        code: i32,
        message: String,
        data: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Create error response
        let error = JSONRPCMessage::Error(crate::schema::json_rpc::JSONRPCError::new_with_details(
            id, code, message, data,
        ));

        // Send the error
        transport.send(&error).await?;

        Ok(())
    }

    /// Execute multiple tools concurrently
    ///
    /// This method allows calling multiple tools at once and gathering their results.
    /// Each call is processed concurrently in its own task.
    pub async fn execute_tools_concurrently(
        &self,
        tool_calls: Vec<(String, Value)>,
    ) -> Vec<Result<Value, MCPError>> {
        let tool_handlers = self.tool_handlers.lock().await;

        let mut futures = Vec::with_capacity(tool_calls.len());

        for (tool_name, params) in tool_calls {
            if let Some(handler) = tool_handlers.get(&tool_name) {
                let future = handler(params);
                futures.push(future);
            } else {
                futures.push(Box::pin(async move {
                    Err(MCPError::Protocol(format!(
                        "No handler registered for tool '{}'",
                        tool_name
                    )))
                }));
            }
        }

        drop(tool_handlers); // Release the lock before awaiting

        join_all(futures).await
    }

    /// Handle ping request
    async fn handle_ping(&mut self, id: RequestId) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Send a simple response for the ping
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        });

        transport.send(&response).await?;
        debug!("Sent ping response");

        Ok(())
    }
}

/// Handler struct for concurrent tool call processing
struct ToolCallHandler<T: Transport + Send + Sync> {
    tool_handlers: Arc<Mutex<HashMap<String, AsyncToolHandler>>>,
    transport: Option<T>,
}

impl<T: Transport + Send + Sync> ToolCallHandler<T>
where
    T: Clone,
{
    /// Handle tools/call request concurrently
    async fn handle_tools_call(
        &self,
        id: RequestId,
        params: Option<Value>,
    ) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Extract the parameters
        let params = params.ok_or_else(|| {
            MCPError::Protocol("Missing parameters in tools/call request".to_string())
        })?;

        // Get the tool name - support both standard MCP ('name' with 'arguments') and our custom format
        let (tool_name, tool_params) = if let Some(name) = params.get("name") {
            let name_str = name
                .as_str()
                .ok_or_else(|| MCPError::Protocol("Tool name must be a string".to_string()))?;

            // Check for 'arguments' (MCP standard) or fall back to 'parameters' (our custom format)
            let params_value = if params.get("arguments").is_some() {
                // MCP standard format: use 'arguments'
                match params.get("arguments") {
                    Some(args) => args.clone(),
                    None => Value::Null,
                }
            } else if params.get("parameters").is_some() {
                // Our custom format: use 'parameters'
                match params.get("parameters") {
                    Some(args) => args.clone(),
                    None => Value::Null,
                }
            } else {
                // No parameters found
                Value::Null
            };

            (name_str.to_string(), params_value)
        } else {
            // Backward compatibility - try to parse as CallToolParams for direct compatibility
            match serde_json::from_value::<CallToolParams>(params.clone()) {
                Ok(call_params) => {
                    let tool_name = call_params.name.clone();
                    let tool_params = match call_params.arguments {
                        Some(args) => serde_json::to_value(args).unwrap_or(Value::Null),
                        None => Value::Null,
                    };
                    (tool_name, tool_params)
                }
                Err(e) => {
                    return Err(MCPError::Protocol(format!(
                        "Invalid tool call parameters: {}",
                        e
                    )));
                }
            }
        };

        debug!(
            "Executing tool {} with params: {:?}",
            tool_name, tool_params
        );

        // Run the tool handler
        let result = self.execute_tool(&tool_name, tool_params).await;

        // Process the result
        match result {
            Ok(result) => {
                // Create a response with the tool result in standard CallToolResult format
                let tool_result = CallToolResult {
                    content: vec![ToolResultContent::Text(
                        crate::schema::common::TextContent {
                            r#type: "text".to_string(),
                            text: serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| format!("{:?}", result)),
                            annotations: None,
                        },
                    )],
                    is_error: None,
                };

                // Create response
                let response = JSONRPCResponse::new(
                    id,
                    serde_json::to_value(tool_result)
                        .map_err(|e| MCPError::Serialization(e.to_string()))?,
                );

                // Send the response
                let mut transport_clone = transport.clone();
                transport_clone
                    .send(&JSONRPCMessage::Response(response))
                    .await?;
            }
            Err(e) => {
                // Create error response
                let error =
                    JSONRPCMessage::Error(crate::schema::json_rpc::JSONRPCError::new_with_details(
                        id,
                        -32000,
                        format!("Tool execution failed: {}", e),
                        None,
                    ));

                // Send the error
                let mut transport_clone = transport.clone();
                transport_clone.send(&error).await?;
            }
        }

        Ok(())
    }

    /// Execute a tool by name
    async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, MCPError> {
        // Get the handler from the map
        let handlers = self.tool_handlers.lock().await;

        // Find the handler
        if let Some(handler) = handlers.get(tool_name) {
            // Execute the handler and return its result
            let future = handler(params);
            drop(handlers); // Release the lock before awaiting
            future.await
        } else {
            // Handler not found
            Err(MCPError::Protocol(format!(
                "No handler registered for tool '{}'",
                tool_name
            )))
        }
    }
}

impl<T: Transport + Send + Sync> Clone for ToolCallHandler<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            tool_handlers: self.tool_handlers.clone(),
            transport: self.transport.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schema::{
            common::ToolInputSchema,
            json_rpc::{JSONRPCMessage, JSONRPCRequest},
            PromptArgument,
        },
        transport::Transport,
    };
    use async_trait::async_trait;
    use futures::Future;
    use serde::{de::DeserializeOwned, Serialize};
    use std::{collections::VecDeque, sync::Arc};
    use tokio::sync::Mutex;

    // Mock transport for testing
    #[derive(Clone)]
    struct MockTransport {
        send_queue: Arc<Mutex<VecDeque<String>>>,
        receive_queue: Arc<Mutex<VecDeque<String>>>,
        is_started: Arc<Mutex<bool>>,
        is_closed: Arc<Mutex<bool>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                send_queue: Arc::new(Mutex::new(VecDeque::new())),
                receive_queue: Arc::new(Mutex::new(VecDeque::new())),
                is_started: Arc::new(Mutex::new(false)),
                is_closed: Arc::new(Mutex::new(false)),
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
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn start(&mut self) -> Result<(), MCPError> {
            let mut started = self.is_started.lock().await;
            *started = true;
            Ok(())
        }

        async fn send<T: Serialize + Send + Sync>(&mut self, message: &T) -> Result<(), MCPError> {
            let serialized = serde_json::to_string(message)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            let mut queue = self.send_queue.lock().await;
            queue.push_back(serialized);
            Ok(())
        }

        async fn receive<T: DeserializeOwned + Send + Sync>(&mut self) -> Result<T, MCPError> {
            let mut queue = self.receive_queue.lock().await;

            if let Some(message) = queue.pop_front() {
                serde_json::from_str(&message).map_err(|e| MCPError::Serialization(e.to_string()))
            } else {
                // In a real implementation, this would block until a message is received
                // For testing, we'll just simulate a timeout/error
                Err(MCPError::Transport("No messages available".to_string()))
            }
        }

        async fn close(&mut self) -> Result<(), MCPError> {
            let mut closed = self.is_closed.lock().await;
            *closed = true;
            Ok(())
        }

        fn set_on_close(&mut self, _callback: Option<crate::transport::CloseCallback>) {
            // Not used in tests
        }

        fn set_on_error(&mut self, _callback: Option<crate::transport::ErrorCallback>) {
            // Not used in tests
        }

        fn set_on_message<F>(&mut self, _callback: Option<F>)
        where
            F: Fn(&str) + Send + Sync + 'static,
        {
            // Not used in tests
        }
    }

    // Helper to run a test with a server with custom configuration
    async fn with_test_server_config<F, Fut>(config: ServerConfig, test: F) -> Result<(), MCPError>
    where
        F: FnOnce(Server<MockTransport>, MockTransport) -> Fut,
        Fut: Future<Output = Result<(), MCPError>>,
    {
        // Create server with provided config
        let mut server = Server::new(config);

        // Register handlers
        if !server.config.tools.is_empty() {
            server.register_tool_handler("echo", |params: Value| async move {
                let message = params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MCPError::Protocol("Missing message parameter".to_string()))?;

                Ok(serde_json::json!({
                    "result": message
                }))
            })?;
        }

        // Create mock transport
        let transport = MockTransport::new();
        let transport_clone = transport.clone();

        // Create a separate task to run the server first
        let mut server_clone = server.clone();
        let server_transport = transport.clone();

        let server_handle = tokio::spawn(async move {
            let _ = server_clone.serve(server_transport).await;
        });

        // Give the server time to start up
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Run the test with the server
        let test_result = test(server, transport_clone).await;

        // Queue a shutdown message
        let shutdown_transport = transport.clone();
        shutdown_transport
            .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                RequestId::Number(999),
                "shutdown".to_string(),
                None,
            )))
            .await;

        // Give the server time to process the shutdown
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Wait for the server task to complete
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), server_handle).await;

        test_result
    }

    // Helper to run a test with a default server configuration
    async fn with_test_server<F, Fut>(test: F) -> Result<(), MCPError>
    where
        F: FnOnce(Server<MockTransport>, MockTransport) -> Fut,
        Fut: Future<Output = Result<(), MCPError>>,
    {
        // Create server config
        let config = ServerConfig::new()
            .with_name("TestServer")
            .with_version("1.0.0")
            .with_tool(Tool {
                name: "echo".to_string(),
                description: Some("Echo tool".to_string()),
                input_schema: ToolInputSchema {
                    r#type: "object".to_string(),
                    properties: Some(
                        [(
                            "message".to_string(),
                            serde_json::json!({
                                "type": "string",
                                "description": "Message to echo"
                            }),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                    required: Some(vec!["message".to_string()]),
                },
            });

        with_test_server_config(config, test).await
    }

    #[tokio::test]
    async fn test_server_initialization() -> Result<(), MCPError> {
        with_test_server(|_server, transport| async move {
            // Queue initialization request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Give server time to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check response
            let response = transport
                .get_last_sent()
                .await
                .ok_or_else(|| MCPError::Protocol("No response received".to_string()))?;

            // Parse response and verify it contains expected data
            let parsed: JSONRPCMessage = serde_json::from_str(&response)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            match parsed {
                JSONRPCMessage::Response(resp) => {
                    // Verify the response has expected fields
                    assert_eq!(resp.id, RequestId::Number(1));

                    // Print the response structure for debugging
                    println!(
                        "Response: {}",
                        serde_json::to_string_pretty(&resp.result).unwrap()
                    );

                    // Check for protocolVersion field (camelCase as in the actual response)
                    let protocol_version = resp
                        .result
                        .get("protocolVersion")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            MCPError::Protocol("Missing protocolVersion string".to_string())
                        })?;
                    assert_eq!(protocol_version, LATEST_PROTOCOL_VERSION);

                    // Check for serverInfo field
                    let server_info = resp
                        .result
                        .get("serverInfo")
                        .ok_or_else(|| MCPError::Protocol("Missing serverInfo".to_string()))?;

                    // Check name field inside serverInfo
                    let name = server_info
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            MCPError::Protocol("Missing name in serverInfo".to_string())
                        })?;
                    assert_eq!(name, "TestServer");

                    // Check version field inside serverInfo
                    let version = server_info
                        .get("version")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            MCPError::Protocol("Missing version in serverInfo".to_string())
                        })?;
                    assert_eq!(version, "1.0.0");

                    // Verify tools capability is present
                    assert!(
                        resp.result
                            .get("capabilities")
                            .and_then(|c| c.get("tools"))
                            .is_some(),
                        "Tools capability missing"
                    );

                    Ok(())
                }
                _ => Err(MCPError::Protocol("Expected response message".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_tools_list() -> Result<(), MCPError> {
        with_test_server(|_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue tools/list request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "tools/list".to_string(),
                    None,
                )))
                .await;

            // Give server time to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check response
            let response = transport
                .get_last_sent()
                .await
                .ok_or_else(|| MCPError::Protocol("No response received".to_string()))?;

            // Parse response and verify it contains expected data
            let parsed: JSONRPCMessage = serde_json::from_str(&response)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            match parsed {
                JSONRPCMessage::Response(resp) => {
                    // Verify the response has expected fields
                    assert_eq!(resp.id, RequestId::Number(2));

                    let tools = resp
                        .result
                        .get("tools")
                        .and_then(|t| t.as_array())
                        .ok_or_else(|| MCPError::Protocol("Missing tools array".to_string()))?;

                    // Verify we have the expected tool
                    assert_eq!(tools.len(), 1, "Expected 1 tool");

                    let tool = &tools[0];
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .ok_or_else(|| MCPError::Protocol("Missing tool name".to_string()))?;

                    assert_eq!(name, "echo", "Tool should be named 'echo'");

                    Ok(())
                }
                _ => Err(MCPError::Protocol("Expected response message".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_tool_call() -> Result<(), MCPError> {
        with_test_server(|_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue tools/call request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "tools/call".to_string(),
                    Some(serde_json::json!({
                        "name": "echo",
                        "arguments": {
                            "message": "Hello, world!"
                        }
                    })),
                )))
                .await;

            // Give server time to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check response
            let response = transport
                .get_last_sent()
                .await
                .ok_or_else(|| MCPError::Protocol("No response received".to_string()))?;

            // Parse response and verify it contains expected data
            let parsed: JSONRPCMessage = serde_json::from_str(&response)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            match parsed {
                JSONRPCMessage::Response(resp) => {
                    // Verify the response has expected fields
                    assert_eq!(resp.id, RequestId::Number(2));

                    // Check the content field in the response
                    let content = resp
                        .result
                        .get("content")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| MCPError::Protocol("Missing content array".to_string()))?;

                    assert!(!content.is_empty(), "Content array should not be empty");

                    // Extract the text content
                    let text_content = &content[0];
                    let text = text_content
                        .get("text")
                        .and_then(|t| t.as_str())
                        .ok_or_else(|| MCPError::Protocol("Missing text in content".to_string()))?;

                    // Verify the response contains our message
                    assert!(
                        text.contains("Hello, world!"),
                        "Response should contain our message"
                    );

                    Ok(())
                }
                _ => Err(MCPError::Protocol("Expected response message".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_concurrent_tool_calls() -> Result<(), MCPError> {
        with_test_server(|server, _transport| async move {
            // Execute multiple tool calls concurrently
            let tool_calls = vec![
                (
                    "echo".to_string(),
                    serde_json::json!({"message": "Message 1"}),
                ),
                (
                    "echo".to_string(),
                    serde_json::json!({"message": "Message 2"}),
                ),
                (
                    "echo".to_string(),
                    serde_json::json!({"message": "Message 3"}),
                ),
            ];

            // Call the execute_tools_concurrently method
            let results = server.execute_tools_concurrently(tool_calls).await;

            // Verify results
            assert_eq!(results.len(), 3, "Should have 3 results");

            // Check each result
            for (i, result) in results.iter().enumerate() {
                let value = result
                    .as_ref()
                    .map_err(|e| MCPError::Protocol(format!("Tool call failed: {}", e)))?;

                let result_str = value
                    .get("result")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| MCPError::Protocol("Missing result string".to_string()))?;

                let expected = format!("Message {}", i + 1);
                assert_eq!(result_str, expected, "Result should match expected message");
            }

            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn test_ping() -> Result<(), MCPError> {
        with_test_server(|_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue ping request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "ping".to_string(),
                    None,
                )))
                .await;

            // Give server time to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check response
            let response = transport
                .get_last_sent()
                .await
                .ok_or_else(|| MCPError::Protocol("No response received".to_string()))?;

            // Parse response and verify it contains expected data
            let parsed: JSONRPCMessage = serde_json::from_str(&response)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            match parsed {
                JSONRPCMessage::Response(resp) => {
                    // Verify the response has the correct ID
                    assert_eq!(resp.id, RequestId::Number(2));

                    // Verify the result is an empty object
                    assert!(resp.result.is_object(), "Result should be an object");
                    assert_eq!(
                        resp.result.as_object().unwrap().len(),
                        0,
                        "Result should be an empty object"
                    );

                    Ok(())
                }
                _ => Err(MCPError::Protocol("Expected response message".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_cancel_request() -> Result<(), MCPError> {
        with_test_server(|_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue cancel request
            let request_id_to_cancel = RequestId::Number(999); // ID of the request to cancel
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "$/cancelRequest".to_string(),
                    Some(serde_json::json!({
                        "id": request_id_to_cancel
                    })),
                )))
                .await;

            // Give server time to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check response
            let response = transport
                .get_last_sent()
                .await
                .ok_or_else(|| MCPError::Protocol("No response received".to_string()))?;

            // Parse response and verify it contains expected data
            let parsed: JSONRPCMessage = serde_json::from_str(&response)
                .map_err(|e| MCPError::Serialization(e.to_string()))?;

            match parsed {
                JSONRPCMessage::Response(resp) => {
                    // Verify the response has the correct ID
                    assert_eq!(resp.id, RequestId::Number(2));

                    // Verify the result is null, indicating success
                    assert!(resp.result.is_null(), "Result should be null");

                    Ok(())
                }
                _ => Err(MCPError::Protocol("Expected response message".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_prompts_list() -> Result<(), MCPError> {
        // Create a config with a test prompt
        let config = ServerConfig::new()
            .with_name("TestServer")
            .with_version("1.0.0")
            .with_prompt(Prompt {
                name: "test_prompt".to_string(),
                description: Some("A test prompt".to_string()),
                arguments: Some(vec![PromptArgument {
                    name: "param1".to_string(),
                    description: Some("Test parameter".to_string()),
                    required: Some(true),
                }]),
            });

        with_test_server_config(config, |_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue prompts/list request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "prompts/list".to_string(),
                    None,
                )))
                .await;

            // Wait for prompts/list to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check the response for prompts/list
            match transport.get_last_sent().await {
                Some(msg) => {
                    let parsed: JSONRPCMessage = serde_json::from_str(&msg)?;

                    if let JSONRPCMessage::Response(resp) = parsed {
                        assert_eq!(resp.id, RequestId::Number(2));

                        // Parse the result field as ListPromptsResult
                        let result: ListPromptsResult = serde_json::from_value(resp.result)?;

                        assert_eq!(result.prompts.len(), 1);
                        assert_eq!(result.prompts[0].name, "test_prompt");
                        assert_eq!(
                            result.prompts[0].description,
                            Some("A test prompt".to_string())
                        );
                        assert!(result.prompts[0].arguments.is_some());
                        assert_eq!(result.prompts[0].arguments.as_ref().unwrap().len(), 1);
                        assert_eq!(
                            result.prompts[0].arguments.as_ref().unwrap()[0].name,
                            "param1"
                        );

                        Ok(())
                    } else {
                        Err(MCPError::Protocol("Expected response message".to_string()))
                    }
                }
                _ => Err(MCPError::Protocol("No response received".to_string())),
            }
        })
        .await
    }

    #[tokio::test]
    async fn test_resources_list() -> Result<(), MCPError> {
        // Create a config with a test resource
        let config = ServerConfig::new()
            .with_name("TestServer")
            .with_version("1.0.0")
            .with_resource(Resource {
                uri: "file:///test/resource.txt".to_string(),
                name: "test_resource".to_string(),
                description: Some("A test resource".to_string()),
                mime_type: Some("text/plain".to_string()),
                size: Some(42),
                annotations: None,
            });

        with_test_server_config(config, |_server, transport| async move {
            // Queue initialization request first (server needs to be initialized)
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(1),
                    "initialize".to_string(),
                    Some(serde_json::json!({
                        "protocol_version": LATEST_PROTOCOL_VERSION
                    })),
                )))
                .await;

            // Wait for initialization to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Discard initialization response
            let _ = transport.get_last_sent().await;

            // Queue resources/list request
            transport
                .queue_message(JSONRPCMessage::Request(JSONRPCRequest::new(
                    RequestId::Number(2),
                    "resources/list".to_string(),
                    None,
                )))
                .await;

            // Wait for resources/list to process
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check the response for resources/list
            match transport.get_last_sent().await {
                Some(msg) => {
                    let parsed: JSONRPCMessage = serde_json::from_str(&msg)?;

                    if let JSONRPCMessage::Response(resp) = parsed {
                        assert_eq!(resp.id, RequestId::Number(2));

                        // Parse the result field as ListResourcesResult
                        let result: ListResourcesResult = serde_json::from_value(resp.result)?;

                        assert_eq!(result.resources.len(), 1);
                        assert_eq!(result.resources[0].name, "test_resource");
                        assert_eq!(
                            result.resources[0].description,
                            Some("A test resource".to_string())
                        );
                        assert_eq!(result.resources[0].uri, "file:///test/resource.txt");
                        assert_eq!(
                            result.resources[0].mime_type,
                            Some("text/plain".to_string())
                        );

                        Ok(())
                    } else {
                        Err(MCPError::Protocol("Expected response message".to_string()))
                    }
                }
                _ => Err(MCPError::Protocol("No response received".to_string())),
            }
        })
        .await
    }
}
