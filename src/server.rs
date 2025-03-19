//! High-level server implementation for MCP

use crate::{
    constants::LATEST_PROTOCOL_VERSION,
    error::MCPError,
    schema::{
        common::Tool,
        json_rpc::{JSONRPCError, JSONRPCMessage, JSONRPCRequest, JSONRPCResponse, RequestId},
        server::CallToolResult,
    },
    tools::ToolsProvider,
    transport::{Transport, TransportExt},
};
use log::{error, info};
use serde_json::Value;
use std::collections::HashMap;

// Trait for tool registration
pub trait ToolRegistration {
    fn register_tool(tools: &mut Vec<crate::schema::common::Tool>);
}

/// Server configuration
pub struct ServerConfig {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Available tools
    pub tools: Vec<Tool>,
}

impl ServerConfig {
    /// Create a new server configuration
    pub fn new() -> Self {
        Self {
            name: "MCP Server".to_string(),
            version: "1.0.0".to_string(),
            tools: Vec::new(),
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
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool handler function type
pub type ToolHandler = Box<dyn Fn(Value) -> Result<Value, MCPError> + Send + Sync>;

/// High-level MCP server
pub struct Server<T: Transport> {
    config: ServerConfig,
    tool_handlers: HashMap<String, ToolHandler>,
    transport: Option<T>,
}

impl<T: Transport> Server<T> {
    /// Create a new MCP server with the given configuration
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            tool_handlers: HashMap::new(),
            transport: None,
        }
    }

    /// Register a tool handler
    pub fn register_tool_handler<F>(&mut self, tool_name: &str, handler: F) -> Result<(), MCPError>
    where
        F: Fn(Value) -> Result<Value, MCPError> + Send + Sync + 'static,
    {
        // Check if the tool exists in the configuration
        if !self.config.tools.iter().any(|t| t.name == tool_name) {
            return Err(MCPError::Protocol(format!(
                "Tool '{}' not found in server configuration",
                tool_name
            )));
        }

        // Register the handler
        self.tool_handlers
            .insert(tool_name.to_string(), Box::new(handler));

        Ok(())
    }

    /// Register tools from a ToolsProvider
    pub fn register_tools_provider<P: ToolsProvider>(
        &mut self,
        provider: &P,
    ) -> Result<(), MCPError> {
        let tools = provider.get_tools();

        for tool in tools {
            self.config.tools.push(tool.clone());
        }

        Ok(())
    }

    /// Process a message from the server implementation
    fn process_messages(&mut self) -> Result<(), MCPError> {
        loop {
            let message = {
                let transport = self
                    .transport
                    .as_mut()
                    .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

                // Receive a message
                match transport.receive() {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        continue;
                    }
                }
            };

            // Handle the message
            match message {
                JSONRPCMessage::Request(request) => {
                    let id = request.id.clone();
                    let _params = request.params.clone();
                    let method = request.method.clone();

                    match method.as_str() {
                        "initialize" => {
                            info!("Received initialization request");
                            // Convert id and params to proper types for handle_initialize
                            let response = self.handle_initialize(&request)?;
                            self.send_response(response)?;
                        }
                        "tool_call" => {
                            info!("Received tool call request");
                            let response = self.process_tool_call(&request)?;
                            self.send_response(response)?;
                        }
                        "shutdown" => {
                            info!("Received shutdown request");
                            let response = JSONRPCResponse::new(id, serde_json::json!({}));
                            self.send_response(response)?;
                            break;
                        }
                        _ => {
                            error!("Unknown method: {}", method);
                            let error_response = JSONRPCError::new(
                                id,
                                -32601,
                                format!("Method not found: {}", method),
                                None,
                            );
                            self.send_error(error_response)?;
                        }
                    }
                }
                _ => {
                    error!("Unexpected message type");
                    continue;
                }
            }
        }

        Ok(())
    }

    // Helper method to send response
    fn send_response(&mut self, response: JSONRPCResponse) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        transport.send(&JSONRPCMessage::Response(response))?;
        Ok(())
    }

    // Helper method to send error
    fn send_error(&mut self, error: JSONRPCError) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        transport.send(&JSONRPCMessage::Error(error))?;
        Ok(())
    }

    /// Start the server with the given transport
    pub fn start(&mut self, mut transport: T) -> Result<(), MCPError> {
        // Start the transport
        transport.start()?;

        // Store the transport
        self.transport = Some(transport);

        // Process messages
        self.process_messages()
    }

    /// Handle initialization request
    fn handle_initialize(&mut self, request: &JSONRPCRequest) -> Result<JSONRPCResponse, MCPError> {
        // Create initialization response
        Ok(JSONRPCResponse::new(
            request.id.clone(),
            serde_json::json!({
                "protocol_version": LATEST_PROTOCOL_VERSION,
                "server_info": {
                    "name": self.config.name,
                    "version": self.config.version
                },
                "tools": self.config.tools,
                "capabilities": {
                    "tools": {
                        "list_changed": false
                    }
                }
            }),
        ))
    }

    /// Handle tool call request
    fn handle_tool_call(&mut self, id: RequestId, params: Option<Value>) -> Result<(), MCPError> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MCPError::Protocol("Transport not initialized".to_string()))?;

        // Extract tool name and parameters
        let params = params.ok_or_else(|| {
            MCPError::Protocol("Missing parameters in tool call request".to_string())
        })?;

        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::Protocol("Missing tool name in parameters".to_string()))?;

        let tool_params = params.get("parameters").cloned().unwrap_or(Value::Null);

        // Find the tool handler
        let handler = self.tool_handlers.get(tool_name).ok_or_else(|| {
            MCPError::Protocol(format!("No handler registered for tool '{}'", tool_name))
        })?;

        // Call the handler
        match handler(tool_params) {
            Ok(result) => {
                // Create tool result response
                let response = JSONRPCResponse::new(
                    id,
                    serde_json::json!({
                        "result": result
                    }),
                );

                // Send the response
                transport.send(&JSONRPCMessage::Response(response))?;
            }
            Err(e) => {
                // Send error response
                self.send_error(JSONRPCError::new(
                    id,
                    -32000,
                    format!("Tool execution failed: {}", e),
                    None,
                ))?;
            }
        }

        Ok(())
    }

    /// Process a tool call with proper mapping to handlers
    fn process_tool_call(&mut self, request: &JSONRPCRequest) -> Result<JSONRPCResponse, MCPError> {
        if let Some(params) = &request.params {
            let tool_name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MCPError::Protocol("Missing tool name in parameters".to_string()))?;

            let parameters = params.get("parameters").cloned().unwrap_or(Value::Null);

            // Find the tool handler
            let handler = self.tool_handlers.get(tool_name).ok_or_else(|| {
                MCPError::Protocol(format!("No handler registered for tool '{}'", tool_name))
            })?;

            // Call the handler
            match handler(parameters) {
                Ok(result) => {
                    // Create tool result response using the CallToolResult schema
                    let tool_result = CallToolResult {
                        content: vec![crate::schema::server::ToolResultContent::Text(
                            crate::schema::common::TextContent {
                                text: result.to_string(),
                                r#type: "text".to_string(),
                                annotations: None,
                            },
                        )],
                        is_error: None,
                    };

                    Ok(JSONRPCResponse::new(
                        request.id.clone(),
                        serde_json::to_value(tool_result).unwrap(),
                    ))
                }
                Err(e) => {
                    // Create error response
                    Err(MCPError::Protocol(format!("Tool execution failed: {}", e)))
                }
            }
        } else {
            Err(MCPError::Protocol(
                "Missing parameters in tool call".to_string(),
            ))
        }
    }

    /// Run method for processing JSON-RPC requests
    pub fn run(&mut self) -> Result<(), MCPError> {
        if self.transport.is_none() {
            return Err(MCPError::Protocol("Transport not initialized".to_string()));
        }

        // Use process_messages for the main loop
        self.process_messages()
    }
}

/// Trait for MCP servers
pub trait ServerTrait {
    /// Start the server
    fn start(&mut self) -> Result<(), MCPError>;

    /// Process incoming requests
    fn run(&mut self) -> Result<(), MCPError>;

    /// Stop the server
    fn stop(&mut self) -> Result<(), MCPError>;
}
