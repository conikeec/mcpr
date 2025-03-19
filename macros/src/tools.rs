use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn, LitStr};

/// Implementation of the mcp_tools macro
pub fn impl_tools_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the name of the struct
    let name = &input.ident;
    let generics = &input.generics;

    // Generate the tools provider implementation
    let expanded = quote! {
        #[derive(Debug, Default)]
        #input

        impl mcpr::tools::ToolsProvider for #name #generics {
            fn get_tools(&self) -> Vec<mcpr::schema::common::Tool> {
                // Default implementation returns an empty list
                // Implementers should override this to provide actual tools
                Vec::new()
            }

            fn execute_tool(&self, name: &str, params: &serde_json::Value) -> Result<serde_json::Value, mcpr::error::MCPError> {
                // Default implementation returns an error for unknown tools
                // Implementers should override this to handle actual tool calls
                Err(mcpr::error::MCPError::NotFound(format!("Tool not found: {}", name)))
            }
        }

        impl #generics #name #generics {
            pub fn new() -> Self {
                Self::default()
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
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_args = &input_fn.sig.inputs;
    let fn_return = &input_fn.sig.output;

    // Generate a method that implements the tool
    let expanded = quote! {
        // Implement the tool method
        #fn_vis fn #fn_name #fn_args #fn_return {
            #fn_block
        }

        // Register the tool schema - fixed space between register_tool_ and function name
        fn register_tool_ #fn_name(&self, tools: &mut Vec<mcpr::schema::common::Tool>) {
            // Get parameter information from the request type
            let mut properties = std::collections::HashMap::new();
            let mut required = Vec::new();

            // Create a schema for this tool
            tools.push(mcpr::schema::common::Tool {
                name: #tool_name.to_string(),
                description: Some(format!("Tool: {}", #tool_name)),
                input_schema: mcpr::schema::common::ToolInputSchema {
                    r#type: "object".to_string(),
                    properties: if !properties.is_empty() { Some(properties) } else { None },
                    required: if !required.is_empty() { Some(required) } else { None },
                },
            });
        }
    };

    TokenStream::from(expanded)
}

/// Create a registry function for tools
#[allow(dead_code)]
pub fn impl_tools_registry(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Ignore attributes for now
    let _attr = attr;

    // Parse the input function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Generate a registry that retrieves and returns all tool schemas
    let expanded = quote! {
        #input_fn
    };

    TokenStream::from(expanded)
}

/// Create a handler function for tool calls
#[allow(dead_code)]
pub fn impl_tools_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Ignore attributes for now
    let _attr = attr;

    // Parse the input function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Generate a handler that dispatches to the appropriate tool function
    let expanded = quote! {
        #input_fn
    };

    TokenStream::from(expanded)
}
