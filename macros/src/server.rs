use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, ItemFn, LitStr};

/// Processes the tool attributes on methods
#[allow(dead_code)]
fn process_tool_attributes(method: &syn::ItemFn) -> Option<proc_macro2::TokenStream> {
    let has_tool_attr = method.attrs.iter().any(|attr| attr.path().is_ident("tool"));

    if has_tool_attr {
        let method_name = &method.sig.ident;
        let method_name_str = method_name.to_string();

        // Extract argument names and types
        let mut arg_names = Vec::new();
        let mut arg_types = Vec::new();
        let mut param_required = Vec::new();
        let mut properties = Vec::new();

        for arg in &method.sig.inputs {
            if let syn::FnArg::Typed(syn::PatType { pat, ty, .. }) = arg {
                if let syn::Pat::Ident(syn::PatIdent { ident, .. }) = &**pat {
                    // Skip 'self' parameter
                    if ident != "self" {
                        let arg_name = ident.to_string();
                        arg_names.push(arg_name.clone());

                        // Store the type as a string
                        let type_str = quote! { #ty }.to_string();
                        arg_types.push(type_str.clone());

                        // Record as required parameter (default true)
                        param_required.push(true);

                        // Generate the property definition
                        properties.push(quote! {
                            let mut property = std::collections::HashMap::new();
                            property.insert("type".to_string(), crate::schema::common::JSONValue {});
                            properties.insert(#arg_name.to_string(), property);
                        });
                    }
                }
            }
        }

        // Build list of required parameters
        let required_params = arg_names
            .iter()
            .zip(param_required.iter())
            .filter_map(|(name, is_required)| {
                if *is_required {
                    Some(quote! { required.push(#name.to_string()); })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Generate the tool registration code
        return Some(quote! {
            // Add this tool to the list of tools
            tools.push(crate::schema::common::Tool {
                name: #method_name_str.to_string(),
                description: Some(format!("Tool: {}", #method_name_str)),
                input_schema: crate::schema::common::ToolInputSchema {
                    r#type: "object".to_string(),
                    properties: {
                        let mut properties = std::collections::HashMap::new();
                        #(#properties)*
                        Some(properties)
                    },
                    required: {
                        let mut required = Vec::new();
                        #(#required_params)*
                        Some(required)
                    },
                },
            });
        });
    }

    None
}

/// Implementation of the mcp_server macro
pub fn impl_server_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the name of the struct
    let name = &input.ident;
    let generics = &input.generics;

    // Generate builder struct name
    let builder_name = Ident::new(&format!("{}Builder", name), Span::call_site());

    // Generate the server implementation with builder pattern
    let expanded = quote! {
        // Define the server struct
        pub struct #name #generics {
            transport: Option<Box<dyn mcpr::transport::Transport + Send>>,
            prompt_provider: Option<Box<dyn mcpr::prompt::PromptProvider + Send + Sync>>,
            resource_provider: Option<Box<dyn mcpr::resource::ResourceProvider + Send + Sync>>,
            tools_provider: Option<Box<dyn mcpr::tools::ToolsProvider + Send + Sync>>,
            server_name: String,
            server_version: String,
            protocol_version: String,
        }

        // Define the builder
        pub struct #builder_name #generics {
            transport: Option<Box<dyn mcpr::transport::Transport + Send>>,
            prompt_provider: Option<Box<dyn mcpr::prompt::PromptProvider + Send + Sync>>,
            resource_provider: Option<Box<dyn mcpr::resource::ResourceProvider + Send + Sync>>,
            tools_provider: Option<Box<dyn mcpr::tools::ToolsProvider + Send + Sync>>,
            protocol_version: String,
            server_name: String,
            server_version: String,
        }

        // Implement builder pattern
        impl #generics #builder_name #generics {
            /// Create a new builder
            pub fn new() -> Self {
                Self {
                    transport: None,
                    prompt_provider: None,
                    resource_provider: None,
                    tools_provider: None,
                    protocol_version: "2023-12-01".to_string(),
                    server_name: String::new(),
                    server_version: String::new(),
                }
            }

            /// Set the transport
            pub fn with_transport<T: mcpr::transport::Transport + Send + 'static>(mut self, transport: T) -> Self {
                self.transport = Some(Box::new(transport) as Box<dyn mcpr::transport::Transport + Send>);
                self
            }

            /// Set the prompt provider
            pub fn with_prompt_provider<P: mcpr::prompt::PromptProvider + Send + Sync + 'static>(mut self, prompt_provider: P) -> Self {
                self.prompt_provider = Some(Box::new(prompt_provider) as Box<dyn mcpr::prompt::PromptProvider + Send + Sync>);
                self
            }

            /// Set the resource provider
            pub fn with_resource_provider<R: mcpr::resource::ResourceProvider + Send + Sync + 'static>(mut self, resource_provider: R) -> Self {
                self.resource_provider = Some(Box::new(resource_provider) as Box<dyn mcpr::resource::ResourceProvider + Send + Sync>);
                self
            }

            /// Set the tools provider
            pub fn with_tools_provider<T: mcpr::tools::ToolsProvider + Send + Sync + 'static>(mut self, tools_provider: T) -> Self {
                self.tools_provider = Some(Box::new(tools_provider) as Box<dyn mcpr::tools::ToolsProvider + Send + Sync>);
                self
            }

            /// Set the protocol version
            pub fn with_protocol_version(mut self, protocol_version: &str) -> Self {
                self.protocol_version = protocol_version.to_string();
                self
            }

            /// Set the server info
            pub fn with_server_info(mut self, name: &str, version: &str) -> Self {
                self.server_name = name.to_string();
                self.server_version = version.to_string();
                self
            }

            /// Build the server
            pub fn build(self) -> #name #generics {
                #name {
                    transport: self.transport,
                    prompt_provider: self.prompt_provider,
                    resource_provider: self.resource_provider,
                    tools_provider: self.tools_provider,
                    protocol_version: self.protocol_version,
                    server_name: self.server_name,
                    server_version: self.server_version,
                }
            }
        }

        // Server implementation
        impl #generics #name #generics {
            pub fn builder() -> #builder_name #generics {
                #builder_name::new()
            }

            /// Start the server and begin processing messages
            pub fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if we have a transport
                let transport = self.transport.as_mut().ok_or_else(|| {
                    mcpr::error::MCPError::Transport("Transport not set".to_string())
                })?;

                // IMPORTANT FIX: Instead of trying to borrow self inside the on_message closure,
                // set up a structure to handle messages without borrowing self:

                // Create a channel for message handling (to avoid borrow conflict)
                let (tx, rx) = std::sync::mpsc::channel::<String>();

                // Clone needed fields for message handling
                let server_name = self.server_name.clone();
                let server_version = self.server_version.clone();
                let protocol_version = self.protocol_version.clone();

                // Clone tools provider, prompt provider, resource provider if available
                let tools_provider = self.tools_provider.clone();
                let prompt_provider = self.prompt_provider.clone();
                let resource_provider = self.resource_provider.clone();

                // Set up message handler to collect messages
                transport.on_message(Box::new(move |msg| {
                    let _ = tx.send(msg.to_string());
                }));

                // Start the transport
                transport.start()?;

                // Before spawning the thread, try to get a cloned transport
                let transport_mutex = std::sync::Arc::new(std::sync::Mutex::new(
                    self.transport.as_ref().map(|t| t.clone())
                ));

                // Start a thread to process messages from the channel
                std::thread::spawn(move || {
                    while let Ok(message) = rx.recv() {
                        log::debug!("Processing message: {}", message);

                        // Parse message and check if it's a valid JSON-RPC message
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&message) {
                            // Extract the method and ID
                            let method = json.get("method").and_then(|m| m.as_str());
                            let id = json.get("id").cloned().unwrap_or(serde_json::Value::Null);
                            let params = json.get("params").cloned();

                            match method {
                                Some("initialize") => {
                                    // Respond with server info
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": {
                                            "name": server_name,
                                            "version": server_version
                                        }
                                    });
                                    if let Ok(transport_guard) = transport_mutex.lock() {
                                        if let Some(transport) = &transport_guard {
                                            let _ = transport.send_json(&response.to_string());
                                        }
                                    }
                                },
                                Some("get_tools") => {
                                    if let Some(tools_ref) = &tools_provider {
                                        let tools = tools_ref.get_tools();
                                        let response = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": {"tools": tools}
                                        });
                                        if let Ok(transport_guard) = transport_mutex.lock() {
                                            if let Some(transport) = &transport_guard {
                                                let _ = transport.send_json(&response.to_string());
                                            }
                                        }
                                    }
                                },
                                Some("call_tool") => {
                                    if let Some(tools_ref) = &tools_provider {
                                        if let Some(params) = params {
                                            let tool_name = params.get("name").and_then(|n| n.as_str());
                                            let tool_params = params.get("parameters").cloned().unwrap_or(serde_json::Value::Null);

                                            if let Some(tool_name) = tool_name {
                                                match tools_ref.execute_tool(tool_name, &tool_params) {
                                                    Ok(result) => {
                                                        let response = serde_json::json!({
                                                            "jsonrpc": "2.0",
                                                            "id": id,
                                                            "result": result
                                                        });
                                                        if let Ok(transport_guard) = transport_mutex.lock() {
                                                            if let Some(transport) = &transport_guard {
                                                                let _ = transport.send_json(&response.to_string());
                                                            }
                                                        }
                                                    },
                                                    Err(e) => {
                                                        let response = serde_json::json!({
                                                            "jsonrpc": "2.0",
                                                            "id": id,
                                                            "error": {
                                                                "code": -32000,
                                                                "message": format!("Tool execution error: {}", e)
                                                            }
                                                        });
                                                        if let Ok(transport_guard) = transport_mutex.lock() {
                                                            if let Some(transport) = &transport_guard {
                                                                let _ = transport.send_json(&response.to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                // Handle other methods similarly
                                _ => {
                                    log::warn!("Unhandled method: {:?}", method);
                                }
                            }
                        }
                    }
                });

                Ok(())
            }

            /// Stop the server
            pub fn stop(&mut self) -> Result<(), mcpr::error::MCPError> {
                // Check if we have a transport
                if let Some(transport) = self.transport.as_mut() {
                    // Close the transport
                    transport.close()?;
                }

                Ok(())
            }

            /// Wait for shutdown (blocking call)
            pub fn wait_for_shutdown(&mut self) -> Result<(), mcpr::error::MCPError> {
                // For stdio transport, we should simply wait for a shutdown signal
                // This doesn't block stdin which is needed for the transport

                // We could use a channel, event, signal, or thread sleep to wait
                // For simplicity, let's just call run() which will process messages
                // in a loop until a shutdown is received
                self.run()
            }

            // Process messages in a loop - FIX HERE to avoid multiple mutable borrows
            pub fn run(&mut self) -> Result<(), mcpr::error::MCPError> {
                if self.transport.is_none() {
                    return Err(mcpr::error::MCPError::Protocol("Transport not initialized".to_string()));
                }

                // Add error handling with retries
                let mut consecutive_errors = 0;
                let max_errors = 5;

                loop {
                    match self.process_single_message() {
                        Ok(true) => {
                            // Message processed successfully, reset error counter
                            consecutive_errors = 0;
                        },
                        Ok(false) => {
                            // No message available, just wait a bit
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        },
                        Err(e) => {
                            consecutive_errors += 1;

                            // Handle different error types differently
                            match e {
                                MCPError::NotConnected | MCPError::AlreadyConnected => {
                                    // These may be temporary, wait and continue
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                },
                                MCPError::Transport(msg) if msg.contains("Connection reset") => {
                                    // Client disconnected, this is normal
                                    consecutive_errors = 0;
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                },
                                _ => {
                                    // Other errors - log and potentially exit if too many
                                    log::error!("Error processing message: {}", e);
                                    if consecutive_errors >= max_errors {
                                        return Err(e);
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                }
                            }
                        }
                    }
                }
            }

            // Modify the process_single_message method in the mcp_server macro
            fn process_single_message(&mut self) -> Result<bool, mcpr::error::MCPError> {
                // For server mode, we don't need to manually receive messages
                // as they're handled by the connection threads
                if let Some(transport) = &mut self.transport {
                    // For server mode, just check if we're still connected
                    if transport.is_connected() {
                        // Sleep a bit to avoid busy waiting
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        return Ok(false); // No message processed, but still connected
                    } else {
                        return Err(mcpr::error::MCPError::NotConnected);
                    }
                } else {
                    return Err(mcpr::error::MCPError::Transport("No transport available".to_string()));
                }
            }

            /// Handle an incoming JSON-RPC message according to the MCP specification
            fn handle_rpc_message(&mut self, message: &str) -> Result<(), MCPError> {
                // Parse the message as JSON
                let json: serde_json::Value = match serde_json::from_str(message) {
                    Ok(json) => json,
                    Err(e) => {
                        // Invalid JSON - send parse error response without ID
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32700,
                                "message": format!("Parse error: {}", e)
                            },
                            "id": null
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol(format!("Parse error: {}", e)));
                    }
                };

                // Extract ID early for error responses
                let id = json.get("id").cloned().unwrap_or(serde_json::Value::Null);

                // Validate JSON-RPC structure
                if !json.is_object() {
                    let response = serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32600,
                            "message": "Invalid request: message is not an object"
                        },
                        "id": id
                    });
                    if let Some(transport) = &mut self.transport {
                        transport.send_json(&response.to_string())?;
                    }
                    return Err(MCPError::Protocol("Invalid request: message is not an object".to_string()));
                }

                // Check for jsonrpc version field
                match json.get("jsonrpc") {
                    Some(version) if version == "2.0" => {},
                    _ => {
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32600,
                                "message": "Invalid request: jsonrpc version must be 2.0"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid request: jsonrpc version must be 2.0".to_string()));
                    }
                }

                // Extract method
                let method = match json.get("method") {
                    Some(m) => match m.as_str() {
                        Some(s) => s,
                        None => {
                            let response = serde_json::json!({
                                "jsonrpc": "2.0",
                                "error": {
                                    "code": -32600,
                                    "message": "Invalid request: method must be a string"
                                },
                                "id": id
                            });
                            if let Some(transport) = &mut self.transport {
                                transport.send_json(&response.to_string())?;
                            }
                            return Err(MCPError::Protocol("Invalid request: method must be a string".to_string()));
                        }
                    },
                    None => {
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32600,
                                "message": "Invalid request: missing method field"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid request: missing method field".to_string()));
                    }
                };

                // Extract params if present (optional in some cases)
                let params = json.get("params").cloned();

                // Process the request based on method
                match method {
                    // MCP protocol methods
                    "initialize" => self.handle_initialize(id)?,
                    "shutdown" => self.handle_shutdown(id)?,

                    // MCP tool-related methods
                    "get_tools" => self.handle_get_tools(id)?,
                    "call_tool" => self.handle_call_tool(id, params)?,

                    // MCP prompt-related methods
                    "get_prompts" => self.handle_get_prompts(id)?,
                    "get_prompt_messages" => self.handle_get_prompt_messages(id, params)?,

                    // MCP resource-related methods
                    "get_resources" => self.handle_get_resources(id)?,
                    "get_resource" => self.handle_get_resource(id, params)?,

                    // Unknown method
                    _ => {
                        // Method not found
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32601,
                                "message": format!("Method not found: {}", method)
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol(format!("Method not found: {}", method)));
                    }
                }

                Ok(())
            }

            /// Handle the "initialize" method
            fn handle_initialize(&mut self, id: serde_json::Value) -> Result<(), MCPError> {
                // Prepare server info response
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "name": self.server_name,
                        "version": self.server_version,
                        "protocol_version": self.protocol_version,
                    }
                });

                // Send the response
                if let Some(transport) = &mut self.transport {
                    transport.send_json(&response.to_string())?;
                }

                Ok(())
            }

            /// Handle the "shutdown" method
            fn handle_shutdown(&mut self, id: serde_json::Value) -> Result<(), MCPError> {
                // Send a success response before shutting down
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                });

                if let Some(transport) = &mut self.transport {
                    transport.send_json(&response.to_string())?;
                }

                // Set a flag to shut down gracefully
                // In a real implementation, you might want to have a shutdown flag
                // self.should_shutdown = true;

                Ok(())
            }

            /// Handle the "get_tools" method
            fn handle_get_tools(&mut self, id: serde_json::Value) -> Result<(), MCPError> {
                let tools = match &self.tools_provider {
                    Some(provider) => provider.get_tools(),
                    None => Vec::new(),
                };

                // Build response
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                });

                if let Some(transport) = &mut self.transport {
                    transport.send_json(&response.to_string())?;
                }

                Ok(())
            }

            /// Handle the "call_tool" method
            fn handle_call_tool(&mut self, id: serde_json::Value, params: Option<serde_json::Value>) -> Result<(), MCPError> {
                // Extract tool name and parameters
                let params = match params {
                    Some(p) => p,
                    None => {
                        // Invalid parameters
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing parameters for call_tool"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing parameters for call_tool".to_string()));
                    }
                };

                // Extract tool name
                let tool_name = match params.get("name").and_then(|n| n.as_str()) {
                    Some(name) => name,
                    None => {
                        // Missing tool name
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing tool name"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing tool name".to_string()));
                    }
                };

                // Extract tool parameters
                let tool_params = params.get("parameters").cloned().unwrap_or(serde_json::Value::Null);

                // Execute the tool
                match &self.tools_provider {
                    Some(provider) => {
                        match provider.execute_tool(tool_name, &tool_params) {
                            Ok(result) => {
                                // Success response
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": result
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                            },
                            Err(e) => {
                                // Tool execution error
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "error": {
                                        "code": -32000, // Application error
                                        "message": format!("Tool execution error: {}", e)
                                    },
                                    "id": id
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                                return Err(e);
                            }
                        }
                    },
                    None => {
                        // No tools provider
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32603, // Internal error
                                "message": "No tools provider available"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Internal("No tools provider available".to_string()));
                    }
                }

                Ok(())
            }

            /// Handle the "get_prompts" method
            fn handle_get_prompts(&mut self, id: serde_json::Value) -> Result<(), MCPError> {
                let prompts = match &self.prompt_provider {
                    Some(provider) => provider.get_prompts(),
                    None => Vec::new()
                };

                // Build response
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "prompts": prompts }
                });

                if let Some(transport) = &mut self.transport {
                    transport.send_json(&response.to_string())?;
                }

                Ok(())
            }

            /// Handle the "get_prompt_messages" method
            fn handle_get_prompt_messages(&mut self, id: serde_json::Value, params: Option<serde_json::Value>) -> Result<(), MCPError> {
                // Extract prompt name
                let params = match params {
                    Some(p) => p,
                    None => {
                        // Invalid parameters
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing parameters for get_prompt_messages"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing parameters for get_prompt_messages".to_string()));
                    }
                };

                // Extract prompt name
                let prompt_name = match params.get("name").and_then(|n| n.as_str()) {
                    Some(name) => name,
                    None => {
                        // Missing prompt name
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing prompt name"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing prompt name".to_string()));
                    }
                };

                // Get prompt messages
                match &self.prompt_provider {
                    Some(provider) => {
                        match provider.get_prompt_messages(prompt_name) {
                            Ok(messages) => {
                                // Success response
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": { "messages": messages }
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                            },
                            Err(e) => {
                                // Prompt provider error
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "error": {
                                        "code": -32000, // Application error
                                        "message": format!("Prompt provider error: {}", e)
                                    },
                                    "id": id
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                                return Err(e);
                            }
                        }
                    },
                    None => {
                        // No prompt provider
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32603, // Internal error
                                "message": "No prompt provider available"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Internal("No prompt provider available".to_string()));
                    }
                }

                Ok(())
            }

            /// Handle the "get_resources" method
            fn handle_get_resources(&mut self, id: serde_json::Value) -> Result<(), MCPError> {
                let resources = match &self.resource_provider {
                    Some(provider) => provider.get_resources(),
                    None => Vec::new()
                };

                // Build response
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "resources": resources }
                });

                if let Some(transport) = &mut self.transport {
                    transport.send_json(&response.to_string())?;
                }

                Ok(())
            }

            /// Handle the "get_resource" method
            fn handle_get_resource(&mut self, id: serde_json::Value, params: Option<serde_json::Value>) -> Result<(), MCPError> {
                // Extract resource URI
                let params = match params {
                    Some(p) => p,
                    None => {
                        // Invalid parameters
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing parameters for get_resource"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing parameters for get_resource".to_string()));
                    }
                };

                // Extract resource URI
                let uri = match params.get("uri").and_then(|u| u.as_str()) {
                    Some(uri) => uri,
                    None => {
                        // Missing resource URI
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32602,
                                "message": "Invalid params: missing resource URI"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Protocol("Invalid params: missing resource URI".to_string()));
                    }
                };

                // Get resource
                match &self.resource_provider {
                    Some(provider) => {
                        match provider.get_resource(uri) {
                            Ok(resource) => {
                                // Success response
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": resource
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                            },
                            Err(e) => {
                                // Resource provider error
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "error": {
                                        "code": -32000, // Application error
                                        "message": format!("Resource provider error: {}", e)
                                    },
                                    "id": id
                                });
                                if let Some(transport) = &mut self.transport {
                                    transport.send_json(&response.to_string())?;
                                }
                                return Err(e);
                            }
                        }
                    },
                    None => {
                        // No resource provider
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32603, // Internal error
                                "message": "No resource provider available"
                            },
                            "id": id
                        });
                        if let Some(transport) = &mut self.transport {
                            transport.send_json(&response.to_string())?;
                        }
                        return Err(MCPError::Internal("No resource provider available".to_string()));
                    }
                }

                Ok(())
            }
        }
    };

    // Convert the generated code back to a TokenStream
    TokenStream::from(expanded)
}

/// Implementation of the tool attribute macro
#[allow(dead_code)]
pub fn tool_attr_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments (tool name)
    let tool_name = parse_macro_input!(attr as LitStr).value();

    // Parse the function definition
    let input_fn = parse_macro_input!(item as ItemFn);
    let _vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let _block = &input_fn.block;
    let _inputs = &input_fn.sig.inputs;
    let _output = &input_fn.sig.output;

    // Create a unique struct name based on the function name
    let struct_name = Ident::new(&format!("{}ToolRegistration", fn_name), Span::call_site());

    // Generate the implementation with tool registration
    let output = quote! {
        // Keep the original method definition intact
        #input_fn

        // Implement a dummy struct for registration
        #[allow(non_camel_case_types)]
        struct #struct_name;

        impl mcpr::server::ToolRegistration for #struct_name {
            fn register_tool(tools: &mut Vec<mcpr::schema::common::Tool>) {
                use std::collections::HashMap;

                let mut properties = HashMap::new();
                let mut required = Vec::new();

                // Add the tool to the registry
                tools.push(mcpr::schema::common::Tool {
                    name: #tool_name.to_string(),
                    description: Some(format!("Tool: {}", #fn_name_str)),
                    input_schema: mcpr::schema::common::ToolInputSchema {
                        r#type: "object".to_string(),
                        properties: Some(properties),
                        required: Some(required),
                    },
                });
            }
        }

        // Create a namespace for this tool
        pub mod #fn_name {
            use super::*;

            pub fn register_tool(tools: &mut Vec<mcpr::schema::common::Tool>) {
                <#struct_name as mcpr::server::ToolRegistration>::register_tool(tools)
            }
        }
    };

    TokenStream::from(output)
}
