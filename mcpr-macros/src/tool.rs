use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitStr};

/// Implementation of the tool attribute macro
pub fn impl_tool_attr_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function
    let input = parse_macro_input!(item as ItemFn);

    // Get the function name and signature
    let fn_name = &input.sig.ident;
    let fn_generics = &input.sig.generics;
    let fn_inputs = &input.sig.inputs;
    let fn_output = &input.sig.output;
    let fn_block = &input.block;

    // Parse tool name from attribute if provided
    let tool_name = if attr.is_empty() {
        // If no name provided, use the function name
        fn_name.to_string()
    } else {
        // Parse the tool name from attribute
        parse_macro_input!(attr as LitStr).value()
    };

    // Generate the tool implementation with registration function that shares the same name
    let expanded = quote! {
        // The actual tool implementation function
        pub fn #fn_name #fn_generics(#fn_inputs) #fn_output {
            #fn_block
        }

        // Creates a namespace for registration functions
        pub mod #fn_name {
            use super::*;

            // A function to register this tool with the tools list
            pub fn register_tool(tools: &mut Vec<mcpr::schema::common::Tool>) {
                tools.push(mcpr::schema::common::Tool {
                    name: #tool_name.to_string(),
                    description: Some(format!("Tool: {}", #tool_name)),
                    input_schema: mcpr::schema::common::ToolInputSchema {
                        r#type: "object".to_string(),
                        properties: Some(std::collections::HashMap::new()),
                        required: None,
                    },
                });
            }
        }
    };

    TokenStream::from(expanded)
}
