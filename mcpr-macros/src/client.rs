use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn, LitStr};

/// Implementation of the mcp_client macro
pub fn impl_client_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the name of the struct
    let name = &input.ident;

    // Generate the client implementation with simplified API
    let expanded = quote! {
        // Define the client struct
        pub struct #name {
            inner: Box<dyn mcpr::ClientInterface>,
            initialized: bool,
            // Store the tools received during initialization
            tools: Vec<mcpr::schema::common::Tool>,
        }

        // Client implementation with simplified API
        impl #name {
            /// Create a new client instance with the given transport
            pub fn new<T: mcpr::transport::Transport + 'static>(transport: T) -> Self {
                let client = mcpr::client::Client::new(transport);
                Self {
                    inner: Box::new(client),
                    initialized: false,
                    tools: Vec::new(),
                }
            }

            /// Initialize the client connection
            /// This must be called before making any other requests
            pub fn initialize(&mut self) -> Result<mcpr::schema::common::Implementation, mcpr::error::MCPError> {
                let response = self.inner.initialize()?;
                self.initialized = true;

                // After successful initialization, try to get the tools list
                match self.inner.call_tool("__get_initialization_tools", &serde_json::Value::Null) {
                    Ok(result) => {
                        // Process the result to extract tools
                        if let Some(tools_value) = result.get("tools") {
                            if let Ok(tools_list) = serde_json::from_value::<Vec<mcpr::schema::common::Tool>>(tools_value.clone()) {
                                self.tools = tools_list;
                            }
                        }
                    },
                    Err(_) => {
                        // Ignore errors - we'll just have an empty tools list
                    }
                }

                Ok(response)
            }

            /// Disconnect from the server
            pub fn disconnect(&mut self) -> Result<(), mcpr::error::MCPError> {
                self.inner.shutdown()
            }

            /// Call a tool by name with parameters
            pub fn call_tool<P: serde::Serialize, R: serde::de::DeserializeOwned>(
                &mut self,
                tool_name: &str,
                parameters: P,
            ) -> Result<R, mcpr::error::MCPError> {
                if !self.initialized {
                    return Err(mcpr::error::MCPError::Protocol(
                        "Client not initialized. Call initialize() first.".to_string()
                    ));
                }

                // Convert parameters to Value
                let params_value = serde_json::to_value(parameters)
                    .map_err(mcpr::error::MCPError::Serialization)?;

                // Call the tool
                let result = self.inner.call_tool(tool_name, &params_value)?;

                // Deserialize the result
                serde_json::from_value(result)
                    .map_err(|e| mcpr::error::MCPError::Deserialization(e))
            }

            /// Get available prompts from the server
            pub fn get_prompts(&mut self) -> Result<Vec<mcpr::schema::common::Prompt>, mcpr::error::MCPError> {
                if !self.initialized {
                    return Err(mcpr::error::MCPError::Protocol(
                        "Client not initialized. Call initialize() first.".to_string()
                    ));
                }
                self.inner.get_prompts()
            }

            /// Get available resources from the server
            pub fn get_resources(&mut self) -> Result<Vec<mcpr::schema::common::Resource>, mcpr::error::MCPError> {
                if !self.initialized {
                    return Err(mcpr::error::MCPError::Protocol(
                        "Client not initialized. Call initialize() first.".to_string()
                    ));
                }
                self.inner.get_resources()
            }

            /// Get a specific resource by URI
            pub fn get_resource(
                &mut self,
                uri: &str,
            ) -> Result<mcpr::schema::common::ResourceContents, mcpr::error::MCPError> {
                if !self.initialized {
                    return Err(mcpr::error::MCPError::Protocol(
                        "Client not initialized. Call initialize() first.".to_string()
                    ));
                }
                self.inner.get_resource(uri)
            }

            /// Get available tools from the server
            pub fn get_tools(&mut self) -> Result<Vec<mcpr::schema::common::Tool>, mcpr::error::MCPError> {
                if !self.initialized {
                    return Err(mcpr::error::MCPError::Protocol(
                        "Client not initialized. Call initialize() first.".to_string()
                    ));
                }

                // Return the cached tools list from initialization
                Ok(self.tools.clone())
            }
        }

        // Implement Debug for the client
        impl std::fmt::Debug for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!(#name))
                    .field("initialized", &self.initialized)
                    .field("tools_count", &self.tools.len())
                    .finish()
            }
        }
    };

    TokenStream::from(expanded)
}

/// Implementation of the tool_call attribute macro
pub fn tool_call_attr_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function
    let input = parse_macro_input!(item as ItemFn);

    // Parse the tool name from the attribute
    let tool_name = parse_macro_input!(attr as LitStr).value();

    // Get the function name
    let fn_name = &input.sig.ident;

    // Get the function return type
    let return_ty = &input.sig.output;

    // Get the function parameters
    let params = &input.sig.inputs;

    // Extract all parameter types, ignoring &self or &mut self
    let param_types: Vec<_> = params
        .iter()
        .filter_map(|param| {
            match param {
                syn::FnArg::Typed(pat_type) => Some(pat_type),
                _ => None, // Skip &self or &mut self
            }
        })
        .collect();

    // Extract parameter names, which are parsed from token trees
    let param_names: Vec<_> = param_types
        .iter()
        .map(|pat_type| match &*pat_type.pat {
            syn::Pat::Ident(pat_ident) => &pat_ident.ident,
            _ => panic!("Expected identifier in parameter"),
        })
        .collect();

    // Generate the implementation
    let expanded = quote! {
        fn #fn_name(#params) #return_ty {
            // Create parameters object
            let parameters = serde_json::json!({
                #(
                    stringify!(#param_names): #param_names,
                )*
            });

            // Call the tool using client.call_tool
            self.call_tool(#tool_name, parameters)
        }
    };

    TokenStream::from(expanded)
}
