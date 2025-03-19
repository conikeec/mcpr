//! High-level client implementation for MCP

use crate::{
    constants::LATEST_PROTOCOL_VERSION,
    error::MCPError,
    schema::{
        client::ClientCapabilities,
        common::{Implementation, Prompt, Resource, ResourceContents},
        json_rpc::{JSONRPCMessage, JSONRPCRequest, RequestId},
    },
    transport::{Transport, TransportExt},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Client for interacting with MCP servers
#[derive(Debug)]
pub struct Client<T: Transport> {
    transport: T,
    server_info: Option<Implementation>,
    protocol_version: String,
    capabilities: ClientCapabilities,
    client_info: Implementation,
    request_id: std::sync::atomic::AtomicUsize,
    _timeout: Duration,
}

impl<T: Transport> Client<T> {
    /// Create a new MCP client with the given transport
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            server_info: None,
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                experimental: None,
                roots: None,
                sampling: None,
            },
            client_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            request_id: std::sync::atomic::AtomicUsize::new(1),
            _timeout: Duration::from_secs(30),
        }
    }

    /// Create a new client with custom configuration
    pub fn with_config(
        transport: T,
        protocol_version: String,
        client_info: Implementation,
        capabilities: ClientCapabilities,
        _timeout: Duration,
    ) -> Self {
        Self {
            transport,
            server_info: None,
            protocol_version,
            capabilities,
            client_info,
            request_id: std::sync::atomic::AtomicUsize::new(1),
            _timeout,
        }
    }

    /// Initialize the client
    pub fn initialize(&mut self) -> Result<Implementation, MCPError> {
        // Start the transport
        self.transport.start()?;

        // Send initialization request
        let initialize_request = JSONRPCRequest::new(
            self.next_request_id(),
            "initialize".to_string(),
            Some(serde_json::json!({
                "protocol_version": self.protocol_version,
                "capabilities": self.capabilities,
                "client_info": self.client_info,
            })),
        );

        let message = JSONRPCMessage::Request(initialize_request);
        self.transport.send(&message)?;

        // Wait for response with timeout
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(resp) => {
                // Extract server info
                let server_info: Implementation = serde_json::from_value(
                    resp.result
                        .get("server_info")
                        .ok_or_else(|| {
                            MCPError::Protocol("Missing server_info in response".to_string())
                        })?
                        .clone(),
                )?;

                self.server_info = Some(server_info.clone());

                // Store the tools from the initialization response (if any)
                let mut stored_tools = Vec::new();
                if let Some(tools_value) = resp.result.get("tools") {
                    if let Ok(tools) = serde_json::from_value::<Vec<crate::schema::common::Tool>>(
                        tools_value.clone(),
                    ) {
                        stored_tools = tools;
                    }
                }

                // Expose these tools in the special tool call
                thread_local! {
                    static INIT_TOOLS: std::cell::RefCell<Vec<crate::schema::common::Tool>> = const { std::cell::RefCell::new(Vec::new()) };
                }

                INIT_TOOLS.with(|tools| {
                    *tools.borrow_mut() = stored_tools;
                });

                Ok(server_info)
            }
            JSONRPCMessage::Error(err) => Err(MCPError::Protocol(format!(
                "Initialization failed: {:?}",
                err
            ))),
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Call a tool on the server
    pub fn call_tool<P: Serialize, R: DeserializeOwned>(
        &mut self,
        tool_name: &str,
        params: &P,
    ) -> Result<R, MCPError> {
        // Special case for getting initialization tools
        if tool_name == "__get_initialization_tools" {
            // Access the stored tools from initialization
            thread_local! {
                static INIT_TOOLS: std::cell::RefCell<Vec<crate::schema::common::Tool>> = const { std::cell::RefCell::new(Vec::new()) };
            }

            let tools_list = INIT_TOOLS.with(|tools| tools.borrow().clone());

            let tools_response = serde_json::json!({
                "tools": tools_list
            });

            return serde_json::from_value(tools_response).map_err(MCPError::Serialization);
        }

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
        self.transport.send(&message)?;

        // Wait for response
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(resp) => {
                // Extract the tool result from the response
                let result_value = resp.result;
                let result = result_value.get("result").ok_or_else(|| {
                    MCPError::Protocol("Missing 'result' field in response".to_string())
                })?;

                // Parse the result
                serde_json::from_value(result.clone()).map_err(MCPError::Serialization)
            }
            JSONRPCMessage::Error(err) => {
                Err(MCPError::Protocol(format!("Tool call failed: {:?}", err)))
            }
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Get available prompts from the server
    pub fn get_prompts(&mut self) -> Result<Vec<crate::schema::common::Prompt>, MCPError> {
        let request = JSONRPCRequest::new(self.next_request_id(), "prompts/list".to_string(), None);

        let message = JSONRPCMessage::Request(request);
        self.transport.send(&message)?;

        // Wait for response
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(resp) => {
                let prompts = resp.result.get("prompts").ok_or_else(|| {
                    MCPError::Protocol("Missing 'prompts' field in response".to_string())
                })?;

                serde_json::from_value(prompts.clone()).map_err(MCPError::Serialization)
            }
            JSONRPCMessage::Error(err) => {
                Err(MCPError::Protocol(format!("Get prompts failed: {:?}", err)))
            }
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Get available resources from the server
    pub fn get_resources(&mut self) -> Result<Vec<crate::schema::common::Resource>, MCPError> {
        let request =
            JSONRPCRequest::new(self.next_request_id(), "resources/list".to_string(), None);

        let message = JSONRPCMessage::Request(request);
        self.transport.send(&message)?;

        // Wait for response
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(resp) => {
                let resources = resp.result.get("resources").ok_or_else(|| {
                    MCPError::Protocol("Missing 'resources' field in response".to_string())
                })?;

                serde_json::from_value(resources.clone()).map_err(MCPError::Serialization)
            }
            JSONRPCMessage::Error(err) => Err(MCPError::Protocol(format!(
                "Get resources failed: {:?}",
                err
            ))),
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Get a specific resource from the server
    pub fn get_resource(
        &mut self,
        uri: &str,
    ) -> Result<crate::schema::common::ResourceContents, MCPError> {
        let request = JSONRPCRequest::new(
            self.next_request_id(),
            "resources/get".to_string(),
            Some(serde_json::json!({
                "uri": uri
            })),
        );

        let message = JSONRPCMessage::Request(request);
        self.transport.send(&message)?;

        // Wait for response
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(resp) => {
                let resource = resp.result.get("resource").ok_or_else(|| {
                    MCPError::Protocol("Missing 'resource' field in response".to_string())
                })?;

                serde_json::from_value(resource.clone()).map_err(MCPError::Serialization)
            }
            JSONRPCMessage::Error(err) => Err(MCPError::Protocol(format!(
                "Get resource failed: {:?}",
                err
            ))),
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Shutdown the client
    pub fn shutdown(&mut self) -> Result<(), MCPError> {
        // Send shutdown request
        let shutdown_request =
            JSONRPCRequest::new(self.next_request_id(), "shutdown".to_string(), None);

        let message = JSONRPCMessage::Request(shutdown_request);
        self.transport.send(&message)?;

        // Wait for response
        let response: JSONRPCMessage = self.transport.receive()?;

        match response {
            JSONRPCMessage::Response(_) => {
                // Close the transport
                self.transport.close()?;
                Ok(())
            }
            JSONRPCMessage::Error(err) => {
                Err(MCPError::Protocol(format!("Shutdown failed: {:?}", err)))
            }
            _ => Err(MCPError::Protocol("Unexpected response type".to_string())),
        }
    }

    /// Generate the next request ID
    fn next_request_id(&mut self) -> RequestId {
        let id = self.request_id.load(std::sync::atomic::Ordering::Relaxed);
        self.request_id
            .store(id + 1, std::sync::atomic::Ordering::Relaxed);
        RequestId::Number(id as i64)
    }
}

/// Trait for MCP clients
pub trait ClientTrait {
    /// Connect to the server and initialize the connection
    fn connect(&mut self) -> Result<Implementation, MCPError>;

    /// Disconnect from the server
    fn disconnect(&mut self) -> Result<(), MCPError>;

    /// Call a tool on the server
    fn call_tool<P: Serialize, R: DeserializeOwned>(
        &mut self,
        tool_name: &str,
        parameters: P,
    ) -> Result<R, MCPError>;

    /// Get available prompts from the server
    fn get_prompts(&mut self) -> Result<Vec<crate::schema::common::Prompt>, MCPError>;

    /// Get available resources from the server
    fn get_resources(&mut self) -> Result<Vec<crate::schema::common::Resource>, MCPError>;

    /// Get a specific resource from the server
    fn get_resource(
        &mut self,
        uri: &str,
    ) -> Result<crate::schema::common::ResourceContents, MCPError>;
}

/// Interface for MCP clients (for dynamic dispatch)
pub trait ClientInterface {
    fn initialize(&mut self) -> Result<Implementation, MCPError>;
    fn shutdown(&mut self) -> Result<(), MCPError>;
    fn call_tool(&mut self, name: &str, parameters: &Value) -> Result<Value, MCPError>;
    fn get_prompts(&mut self) -> Result<Vec<Prompt>, MCPError>;
    fn get_resources(&mut self) -> Result<Vec<Resource>, MCPError>;
    fn get_resource(&mut self, uri: &str) -> Result<ResourceContents, MCPError>;
}

// Implement ClientInterface trait for the actual Client<T> struct
impl<T: Transport> ClientInterface for Client<T> {
    fn initialize(&mut self) -> Result<Implementation, MCPError> {
        self.initialize()
    }

    fn shutdown(&mut self) -> Result<(), MCPError> {
        self.shutdown()
    }

    fn call_tool(&mut self, name: &str, parameters: &Value) -> Result<Value, MCPError> {
        self.call_tool(name, parameters)
    }

    fn get_prompts(&mut self) -> Result<Vec<Prompt>, MCPError> {
        self.get_prompts()
    }

    fn get_resources(&mut self) -> Result<Vec<Resource>, MCPError> {
        self.get_resources()
    }

    fn get_resource(&mut self, uri: &str) -> Result<ResourceContents, MCPError> {
        self.get_resource(uri)
    }
}

// Add implementation of ClientInterface trait for Box<dyn ClientInterface>
impl ClientInterface for Box<dyn ClientInterface> {
    fn initialize(&mut self) -> Result<Implementation, MCPError> {
        (**self).initialize()
    }

    fn shutdown(&mut self) -> Result<(), MCPError> {
        (**self).shutdown()
    }

    fn call_tool(&mut self, name: &str, parameters: &Value) -> Result<Value, MCPError> {
        (**self).call_tool(name, parameters)
    }

    fn get_prompts(&mut self) -> Result<Vec<Prompt>, MCPError> {
        (**self).get_prompts()
    }

    fn get_resources(&mut self) -> Result<Vec<Resource>, MCPError> {
        (**self).get_resources()
    }

    fn get_resource(&mut self, uri: &str) -> Result<ResourceContents, MCPError> {
        (**self).get_resource(uri)
    }
}
