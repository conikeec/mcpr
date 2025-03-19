use proc_macro::TokenStream;

use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, PatIdent,
    PatType
};

/// Processes the auth attributes on methods
#[allow(dead_code)]
fn process_auth_attributes(method: &ImplItemFn) -> Option<proc_macro2::TokenStream> {
    for attr in &method.attrs {
        if attr.path().is_ident("auth") {
            let method_name = &method.sig.ident;
            let method_name_str = method_name.to_string();

            // Extract method parameters for auth definition
            let mut arg_defs = Vec::new();

            for arg in &method.sig.inputs {
                if let FnArg::Typed(PatType { pat, ty: _, .. }) = arg {
                    if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                        // Skip 'self' parameter
                        if ident != "self" {
                            let arg_name = ident.to_string();
                            arg_defs.push(quote! {
                                #arg_name.to_string()
                            });
                        }
                    }
                }
            }

            // Generate the auth registration code
            return Some(quote! {
                // Add this auth method to the list of auth methods
                auth_methods.push(::mcpr::schema::auth::AuthMethod {
                    name: #method_name_str.to_string(),
                    description: Some(format!("Authentication method: {}", #method_name_str)),
                    params: vec![#(#arg_defs),*],
                    auth_type: ::mcpr::schema::auth::AuthType::BearerToken,
                });
            });
        }
    }
    None
}

/// Implementation of the mcp_auth macro
pub fn impl_auth_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let impl_type = &input.self_ty;
    let trait_path = input.trait_.clone().map(|(path, _, _)| path);

    // Store auth methods we find
    let mut auth_methods = Vec::new();

    // Prepare new implementation items list
    let mut items = Vec::new();
    
    // Check if auth_methods method already exists before moving items
    let has_auth_methods_method = input.items.iter().any(|item| {
        if let ImplItem::Fn(method) = item {
            method.sig.ident == "auth_methods"
        } else {
            false
        }
    });

    // Process each item in the implementation
    for item in input.items {
        if let ImplItem::Fn(mut method) = item {
            let has_auth_attr = method.attrs.iter().any(|attr| attr.path().is_ident("auth"));
            
            if has_auth_attr {
                // Extract method details
                let method_name = &method.sig.ident;
                let _method_span = method.sig.span();
                
                // Analyze method parameters to extract auth params
                let mut auth_params = Vec::new();
                for arg in &method.sig.inputs {
                    if let FnArg::Typed(PatType { pat, ty: _, .. }) = arg {
                        if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                            auth_params.push(ident.clone());
                        }
                    }
                }
                
                // Add this method to our auth methods list
                auth_methods.push((method_name.clone(), auth_params));
                
                // Remove auth attribute to avoid attribute processing conflicts
                method.attrs.retain(|attr| !attr.path().is_ident("auth"));
                
                items.push(ImplItem::Fn(method));
            } else {
                items.push(ImplItem::Fn(method));
            }
        } else {
            items.push(item);
        }
    }

    // Check if we need to generate auth_methods
    let has_auth_methods = !auth_methods.is_empty();
    let needs_auth_methods_list = has_auth_methods && !has_auth_methods_method;

    // Generate auth_methods method if needed
    if has_auth_methods && needs_auth_methods_list {
        let auth_methods_decl = generate_auth_methods_list(&auth_methods);
        items.push(ImplItem::Fn(auth_methods_decl));
    }

    // Reconstruct the implementation with our modifications
    let result = quote! {
        #[automatically_derived]
        impl #trait_path #impl_type {
            #(#items)*
        }
    };

    result.into()
}

/// Generate a method that returns a list of auth methods with their parameter names
fn generate_auth_methods_list(
    auth_methods: &[(syn::Ident, Vec<syn::Ident>)],
) -> ImplItemFn {
    let auth_methods_body = auth_methods.iter().map(|(method_name, params)| {
        let param_names = params.iter().map(|param| {
            quote! { #param.to_string() }
        });
        
        quote! {
            auth_methods.push(::mcpr::schema::auth::AuthMethod {
                name: stringify!(#method_name).to_string(),
                description: None,
                params: vec![#(#param_names),*],
                auth_type: ::mcpr::schema::auth::AuthType::Custom(stringify!(#method_name).to_string())
            });
        }
    });

    // Parse the method declaration
    syn::parse_quote! {
        fn auth_methods(&self) -> Vec<::mcpr::schema::auth::AuthMethod> {
            let mut auth_methods = Vec::new();
            #(#auth_methods_body)*
            auth_methods
        }
    }
}

/// Implementation of the auth attribute macro
pub fn auth_attr_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // For now, just return the original implementation unchanged
    // The auth attribute will be processed by the mcp_auth macro
    item
}
