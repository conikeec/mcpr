use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, FnArg, Ident, ImplItem, ImplItemFn, ItemFn,
    ItemImpl, LitStr, Pat, PatIdent, PatType,
};

/// Processes the resource attributes on methods
fn process_resource_attributes(method: &ImplItemFn) -> Option<proc_macro2::TokenStream> {
    for attr in &method.attrs {
        if attr.path().is_ident("resource") {
            let method_name = &method.sig.ident;
            let method_name_str = method_name.to_string();

            // Extract argument names and types
            let mut arg_definitions = Vec::new();

            for arg in &method.sig.inputs {
                if let FnArg::Typed(PatType { pat, ty: _, .. }) = arg {
                    if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                        // Skip 'self' parameter
                        if ident != "self" {
                            let arg_name = ident.to_string();

                            // Generate uri template parameter
                            arg_definitions.push(quote! {
                                #arg_name.to_string()
                            });
                        }
                    }
                }
            }

            // Generate the resource registration code
            return Some(quote! {
                // Add this resource to the list of resources
                resources.push(::mcpr::schema::common::ResourceTemplate {
                    uri_template: format!("/resources/{}{}", #method_name_str,
                        if vec![#(#arg_definitions),*].is_empty() {
                            "".to_string()
                        } else {
                            format!("/{{{}}}", vec![#(#arg_definitions),*].join("}/{"))
                        }
                    ),
                    name: #method_name_str.to_string(),
                    description: Some(format!("Resource: {}", #method_name_str)),
                    mime_type: Some("application/json".to_string()),
                    annotations: None,
                });
            });
        }
    }
    None
}

/// Implementation of the mcp_resource macro
pub fn impl_resource_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
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

/// Implementation of the resource attribute macro
pub fn resource_attr_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
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
