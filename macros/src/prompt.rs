use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, FnArg, ImplItemFn, ItemFn, Pat, PatIdent, PatType};

/// Processes the prompt attributes on methods
#[allow(dead_code)]
fn process_prompt_attributes(method: &ImplItemFn) -> Option<proc_macro2::TokenStream> {
    for attr in &method.attrs {
        if attr.path().is_ident("prompt") {
            let method_name = &method.sig.ident;
            let method_name_str = method_name.to_string();

            // Extract parameter names and descriptions
            let mut param_defs = Vec::new();

            for arg in &method.sig.inputs {
                if let FnArg::Typed(PatType { pat, ty: _, .. }) = arg {
                    if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                        // Skip 'self' parameter
                        if ident != "self" {
                            let arg_name = ident.to_string();
                            param_defs.push(quote! {
                                ::mcpr::schema::common::PromptArgument {
                                    name: #arg_name.to_string(),
                                    description: Some(format!("Parameter {} for prompt {}", #arg_name, #method_name_str)),
                                    required: Some(true),
                                }
                            });
                        }
                    }
                }
            }

            // Generate the prompt registration code
            return Some(quote! {
                // Add this prompt to the list of prompts
                prompts.push(::mcpr::schema::common::Prompt {
                    name: #method_name_str.to_string(),
                    description: Some(format!("Prompt: {}", #method_name_str)),
                    arguments: Some(vec![#(#param_defs),*]),
                });
            });
        }
    }
    None
}

/// Implementation of the mcp_prompt macro
pub fn impl_prompt_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the name of the struct
    let name = &input.ident;
    let generics = &input.generics;

    // Only generate the struct definition without the trait implementation
    let expanded = quote! {
        #[derive(Debug, Default)]
        #input

        // Define the builder struct for easier construction
        impl #name #generics {
            pub fn new() -> Self {
                Self::default()
            }
        }
    };

    // Convert the generated code back to a TokenStream
    TokenStream::from(expanded)
}

/// Implementation of the prompt attribute macro
pub fn prompt_attr_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the function definition
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_args = &input_fn.sig.inputs;
    let fn_return = &input_fn.sig.output;

    // Generate a method that conforms to the function signature
    let expanded = quote! {
        #fn_vis fn #fn_name(#fn_args) #fn_return {
            #fn_block
        }
    };

    TokenStream::from(expanded)
}
