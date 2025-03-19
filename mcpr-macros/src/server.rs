use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, ItemFn, LitStr};

/// Processes the tool attributes on methods
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
            transport: Option<Box<dyn mcpr::transport::Transport>>,
            prompt_provider: Option<Box<dyn mcpr::prompt::PromptProvider>>,
            resource_provider: Option<Box<dyn mcpr::resource::ResourceProvider>>,
            tools_provider: Option<Box<dyn mcpr::tools::ToolsProvider>>,
            server_name: String,
            server_version: String,
            protocol_version: String,
        }

        // Define the builder
        pub struct #builder_name #generics {
            transport: Option<Box<dyn mcpr::transport::Transport>>,
            prompt_provider: Option<Box<dyn mcpr::prompt::PromptProvider>>,
            resource_provider: Option<Box<dyn mcpr::resource::ResourceProvider>>,
            tools_provider: Option<Box<dyn mcpr::tools::ToolsProvider>>,
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
            pub fn with_transport<T: mcpr::transport::Transport + 'static>(mut self, transport: T) -> Self {
                self.transport = Some(Box::new(transport) as Box<dyn mcpr::transport::Transport>);
                self
            }

            /// Set the prompt provider
            pub fn with_prompt_provider<P: mcpr::prompt::PromptProvider + 'static>(mut self, prompt_provider: P) -> Self {
                self.prompt_provider = Some(Box::new(prompt_provider) as Box<dyn mcpr::prompt::PromptProvider>);
                self
            }

            /// Set the resource provider
            pub fn with_resource_provider<R: mcpr::resource::ResourceProvider + 'static>(mut self, resource_provider: R) -> Self {
                self.resource_provider = Some(Box::new(resource_provider) as Box<dyn mcpr::resource::ResourceProvider>);
                self
            }

            /// Set the tools provider
            pub fn with_tools_provider<T: mcpr::tools::ToolsProvider + 'static>(mut self, tools_provider: T) -> Self {
                self.tools_provider = Some(Box::new(tools_provider) as Box<dyn mcpr::tools::ToolsProvider>);
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

                // Start the transport
                transport.start()?;

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
                self.start()?;

                // Use a local running flag
                let mut running = true;

                // Extract transport once before the loop
                let transport = self.transport.as_mut().ok_or_else(||
                    mcpr::error::MCPError::Transport("Transport not available".to_string())
                )?;

                while running {
                    // Receive a message
                    match transport.receive_json() {
                        Ok(message_text) => {
                            // Process the message text
                            match serde_json::from_str::<serde_json::Value>(&message_text) {
                                Ok(json_message) => {
                                    // First, determine what type of message it is
                                    if let Ok(request) = serde_json::from_value::<mcpr::schema::json_rpc::JSONRPCRequest>(json_message.clone()) {
                                        let method = request.method.as_str();
                                        let id = request.id.clone();

                                        // Handle different method types
                                        match method {
                                            "shutdown" => {
                                                // Send shutdown response
                                                let response = mcpr::schema::json_rpc::JSONRPCResponse {
                                                    jsonrpc: "2.0".to_string(),
                                                    id,
                                                    result: serde_json::json!({}),
                                                };
                                                transport.send_json(&serde_json::to_string(&response).unwrap())?;
                                                running = false;
                                            },
                                            "initialize" => {
                                                // Create initialize response with tools list
                                                let mut tools_list = Vec::new();
                                                if let Some(provider) = &self.tools_provider {
                                                    tools_list = provider.get_tools();
                                                }

                                                let response = mcpr::schema::json_rpc::JSONRPCResponse {
                                                    jsonrpc: "2.0".to_string(),
                                                    id,
                                                    result: serde_json::json!({
                                                        "protocol_version": self.protocol_version,
                                                        "server_info": {
                                                            "name": self.server_name,
                                                            "version": self.server_version
                                                        },
                                                        "tools": tools_list
                                                    }),
                                                };
                                                transport.send_json(&serde_json::to_string(&response).unwrap())?;
                                            },
                                            "tool_call" => {
                                                // Handle tool call if we have a tools provider
                                                if let Some(provider) = &self.tools_provider {
                                                    // Extract tool name and parameters
                                                    if let Some(params) = &request.params {
                                                        if let (Some(tool_name), Some(parameters)) = (
                                                            params.get("name").and_then(|v| v.as_str()),
                                                            params.get("parameters")
                                                        ) {
                                                            // Call the tool
                                                            match provider.execute_tool(tool_name, parameters) {
                                                                Ok(result) => {
                                                                    // Send success response
                                                                    let response = mcpr::schema::json_rpc::JSONRPCResponse {
                                                                        jsonrpc: "2.0".to_string(),
                                                                        id,
                                                                        result: serde_json::json!({
                                                                            "result": result
                                                                        }),
                                                                    };
                                                                    transport.send_json(&serde_json::to_string(&response).unwrap())?;
                                                                },
                                                                Err(e) => {
                                                                    // Send error response
                                                                    let error = mcpr::schema::json_rpc::JSONRPCError {
                                                                        jsonrpc: "2.0".to_string(),
                                                                        id,
                                                                        error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                                            code: -32000,
                                                                            message: format!("Tool execution failed: {}", e),
                                                                            data: None,
                                                                        },
                                                                    };
                                                                    transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                                }
                                                            }
                                                        } else {
                                                            // Missing tool name or parameters
                                                            let error = mcpr::schema::json_rpc::JSONRPCError {
                                                                jsonrpc: "2.0".to_string(),
                                                                id,
                                                                error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                                    code: -32602,
                                                                    message: "Invalid parameters: missing tool name or parameters".to_string(),
                                                                    data: None,
                                                                },
                                                            };
                                                            transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                        }
                                                    } else {
                                                        // Missing parameters
                                                        let error = mcpr::schema::json_rpc::JSONRPCError {
                                                            jsonrpc: "2.0".to_string(),
                                                            id,
                                                            error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                                code: -32602,
                                                                message: "Invalid parameters: missing parameters object".to_string(),
                                                                data: None,
                                                            },
                                                        };
                                                        transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                    }
                                                } else {
                                                    // No tools provider registered
                                                    let error = mcpr::schema::json_rpc::JSONRPCError {
                                                        jsonrpc: "2.0".to_string(),
                                                        id,
                                                        error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                            code: -32603,
                                                            message: "No tools provider registered".to_string(),
                                                            data: None,
                                                        },
                                                    };
                                                    transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                }
                                            },
                                            "prompts/list" => {
                                                // Handle prompts list if we have a prompt provider
                                                if let Some(provider) = &self.prompt_provider {
                                                    let prompts = provider.get_prompts();
                                                    // Send success response
                                                    let response = mcpr::schema::json_rpc::JSONRPCResponse {
                                                        jsonrpc: "2.0".to_string(),
                                                        id,
                                                        result: serde_json::json!({
                                                            "prompts": prompts
                                                        }),
                                                    };
                                                    transport.send_json(&serde_json::to_string(&response).unwrap())?;
                                                } else {
                                                    // No prompt provider registered
                                                    let error = mcpr::schema::json_rpc::JSONRPCError {
                                                        jsonrpc: "2.0".to_string(),
                                                        id,
                                                        error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                            code: -32603,
                                                            message: "No prompt provider registered".to_string(),
                                                            data: None,
                                                        },
                                                    };
                                                    transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                }
                                            },
                                            "resources/list" => {
                                                // Handle resources list if we have a resource provider
                                                if let Some(provider) = &self.resource_provider {
                                                    let resources = provider.get_resources();
                                                    // Send success response
                                                    let response = mcpr::schema::json_rpc::JSONRPCResponse {
                                                        jsonrpc: "2.0".to_string(),
                                                        id,
                                                        result: serde_json::json!({
                                                            "resources": resources
                                                        }),
                                                    };
                                                    transport.send_json(&serde_json::to_string(&response).unwrap())?;
                                                } else {
                                                    // No resource provider registered
                                                    let error = mcpr::schema::json_rpc::JSONRPCError {
                                                        jsonrpc: "2.0".to_string(),
                                                        id,
                                                        error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                            code: -32603,
                                                            message: "No resource provider registered".to_string(),
                                                            data: None,
                                                        },
                                                    };
                                                    transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                                }
                                            },
                                            // Other methods would be handled here
                                            _ => {
                                                // Unknown method - send error
                                                let error = mcpr::schema::json_rpc::JSONRPCError {
                                                    jsonrpc: "2.0".to_string(),
                                                    id,
                                                    error: mcpr::schema::json_rpc::JSONRPCErrorObject {
                                                        code: -32601,
                                                        message: format!("Method not found: {}", method),
                                                        data: None,
                                                    },
                                                };
                                                transport.send_json(&serde_json::to_string(&error).unwrap())?;
                                            }
                                        }
                                    } else {
                                        log::error!("Failed to parse message as JSON-RPC request");
                                    }
                                },
                                Err(e) => {
                                    log::error!("Failed to parse message as JSON: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to receive message: {}", e);
                            running = false;
                        }
                    }
                }

                self.stop()?;
                Ok(())
            }
        }
    };

    // Convert the generated code back to a TokenStream
    TokenStream::from(expanded)
}

/// Implementation of the tool attribute macro
pub fn tool_attr_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments (tool name)
    let tool_name = parse_macro_input!(attr as LitStr).value();

    // Parse the function definition
    let input_fn = parse_macro_input!(item as ItemFn);
    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let block = &input_fn.block;
    let inputs = &input_fn.sig.inputs;
    let output = &input_fn.sig.output;

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
