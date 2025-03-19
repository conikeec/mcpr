use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, punctuated::Punctuated, Attribute, Expr, ExprLit, ImplItem,
    ItemFn, ItemImpl, Lit, LitStr, Token,
};

/// A simple DSL for defining state transitions
struct StateConfig {
    states: Vec<StateDefinition>,
    transitions: Vec<TransitionDefinition>,
}

struct StateDefinition {
    name: String,
    deps: Vec<String>,
}

struct TransitionDefinition {
    from: String,
    to: String,
    condition: Option<String>,
}

impl Parse for StateConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut states = Vec::new();
        let mut transitions = Vec::new();

        while !input.is_empty() {
            let lookahead = input.lookahead1();

            if lookahead.peek(syn::Ident) {
                let ident: syn::Ident = input.parse()?;
                let ident_str = ident.to_string();

                match ident_str.as_str() {
                    "state" => {
                        // Parse state definition: state NAME requires [DEP1, DEP2]
                        let name: syn::Ident = input.parse()?;
                        let state_name = name.to_string();

                        let mut deps = Vec::new();
                        if input.peek(syn::Ident) {
                            let kw: syn::Ident = input.parse()?;
                            if kw == "requires" {
                                // Parse dependencies
                                let content;
                                syn::bracketed!(content in input);
                                let deps_punct: Punctuated<syn::Ident, Token![,]> =
                                    Punctuated::parse_terminated(&content)?;

                                for dep in deps_punct {
                                    deps.push(dep.to_string());
                                }
                            }
                        }

                        states.push(StateDefinition {
                            name: state_name,
                            deps,
                        });
                    }
                    "transition" => {
                        // Parse transition: transition FROM -> TO when CONDITION
                        let from: syn::Ident = input.parse()?;
                        let _arrow: Token![->] = input.parse()?;
                        let to: syn::Ident = input.parse()?;

                        let mut condition = None;
                        if input.peek(syn::Ident) {
                            let kw: syn::Ident = input.parse()?;
                            if kw == "when" {
                                let cond_expr: syn::Expr = input.parse()?;
                                if let Expr::Lit(ExprLit {
                                    lit: Lit::Str(lit_str),
                                    ..
                                }) = cond_expr
                                {
                                    condition = Some(lit_str.value());
                                }
                            }
                        }

                        transitions.push(TransitionDefinition {
                            from: from.to_string(),
                            to: to.to_string(),
                            condition,
                        });
                    }
                    _ => {
                        return Err(syn::Error::new(
                            ident.span(),
                            "Expected 'state' or 'transition'",
                        ))
                    }
                }

                // Consume the semicolon
                if input.peek(Token![;]) {
                    let _: Token![;] = input.parse()?;
                }
            } else {
                return Err(lookahead.error());
            }
        }

        Ok(StateConfig {
            states,
            transitions,
        })
    }
}

/// Extract state machine config from attributes
fn extract_state_config(attrs: &[Attribute]) -> Option<StateConfig> {
    for attr in attrs {
        if attr.path().is_ident("states") {
            if let Ok(Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            })) = attr.meta.require_name_value().map(|v| v.value.clone())
            {
                let config_str = s.value();
                return syn::parse_str::<StateConfig>(&config_str).ok();
            }
        }
    }
    None
}

/// Generate state check code
fn generate_state_check(config: &StateConfig) -> proc_macro2::TokenStream {
    let mut state_checks = Vec::new();

    // Generate checks for each state dependency
    for state in &config.states {
        let state_name = syn::Ident::new(&state.name, Span::call_site());
        let dep_checks = state.deps.iter().map(|dep| {
            let dep_name = syn::Ident::new(dep, Span::call_site());
            quote! {
                if !self.state_manager.is_state_active(stringify!(#dep_name)) {
                    return Err(::mcpr::error::MCPError::State(
                        format!("State '{}' requires '{}' to be active", stringify!(#state_name), stringify!(#dep_name))
                    ));
                }
            }
        });

        let check = quote! {
            if self.state_manager.current_state() == stringify!(#state_name) {
                #(#dep_checks)*
                // All dependencies satisfied
            }
        };

        state_checks.push(check);
    }

    // Generate transition checks
    let transition_checks = config.transitions.iter().map(|transition| {
        let from_state = syn::Ident::new(&transition.from, Span::call_site());
        let to_state = syn::Ident::new(&transition.to, Span::call_site());

        let condition_check = if let Some(condition) = &transition.condition {
            let condition_str = condition.clone();
            quote! {
                if #condition_str {
                    // Condition satisfied, allow transition
                    return true;
                }
            }
        } else {
            quote! {
                // No condition, always allow transition
                return true;
            }
        };

        quote! {
            if current_state == stringify!(#from_state) && target_state == stringify!(#to_state) {
                #condition_check
            }
        }
    });

    quote! {
        // Add state manager to the implementation
        impl StateManager {
            pub fn new(initial_state: &str) -> Self {
                Self {
                    current_state: initial_state.to_string(),
                    active_states: vec![initial_state.to_string()],
                }
            }

            pub fn current_state(&self) -> &str {
                &self.current_state
            }

            pub fn is_state_active(&self, state: &str) -> bool {
                self.active_states.contains(&state.to_string())
            }

            pub fn can_transition(&self, target_state: &str) -> bool {
                let current_state = &self.current_state;

                #(#transition_checks)*

                // Default: no valid transition found
                false
            }

            pub fn transition(&mut self, target_state: &str) -> Result<(), ::mcpr::error::MCPError> {
                if self.can_transition(target_state) {
                    self.current_state = target_state.to_string();
                    self.active_states.push(target_state.to_string());
                    Ok(())
                } else {
                    Err(::mcpr::error::MCPError::State(
                        format!("Cannot transition from '{}' to '{}'", self.current_state, target_state)
                    ))
                }
            }

            // Check if all required states are active
            pub fn check_dependencies(&self) -> Result<(), ::mcpr::error::MCPError> {
                #(#state_checks)*
                Ok(())
            }
        }
    }
}

/// Implementation of the mcp_state macro for standalone functions
pub fn impl_state_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Try to parse as a function first
    if let Ok(input) = syn::parse::<ItemFn>(item.clone()) {
        // Extract state configuration from attributes
        let state_config = extract_state_config(&input.attrs).unwrap_or_else(|| {
            // Default minimal configuration if not specified
            StateConfig {
                states: vec![
                    StateDefinition {
                        name: "initial".to_string(),
                        deps: vec![],
                    },
                    StateDefinition {
                        name: "authenticated".to_string(),
                        deps: vec!["initial".to_string()],
                    },
                    StateDefinition {
                        name: "authorized".to_string(),
                        deps: vec!["authenticated".to_string()],
                    },
                ],
                transitions: vec![
                    TransitionDefinition {
                        from: "initial".to_string(),
                        to: "authenticated".to_string(),
                        condition: None,
                    },
                    TransitionDefinition {
                        from: "authenticated".to_string(),
                        to: "authorized".to_string(),
                        condition: None,
                    },
                ],
            }
        });

        // We need to wrap the function with state checks
        let fn_name = input.sig.ident.clone();
        let args = input.sig.inputs.clone();
        let output = input.sig.output.clone();
        let generics = input.sig.generics.clone();
        let visibility = input.vis.clone();
        let original_body = input.block.clone();

        let access_control = quote! {
            // Check if current state meets dependencies
            self.state_manager.check_dependencies()?;
        };

        let wrapped_method = quote! {
            #visibility fn #fn_name #generics(#args) #output {
                #access_control
                #original_body
            }
        };

        // Generate the state manager implementation
        let state_manager_impl = generate_state_check(&state_config);

        // Generate final output
        let expanded = quote! {
            // Define the state manager struct
            pub struct StateManager {
                current_state: String,
                active_states: Vec<String>,
            }

            #state_manager_impl

            #wrapped_method
        };

        TokenStream::from(expanded)
    }
    // If it's not a function, it might be an impl method
    else if let Ok(mut impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        // Look for methods with the #[mcp_state] attribute and apply our transform
        for item in &mut impl_item.items {
            if let ImplItem::Fn(ref mut method) = item {
                // Check if the method has the mcp_state attribute
                let has_state_attr = method
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("mcp_state"));

                if has_state_attr {
                    // Extract state configuration
                    let _state_config = extract_state_config(&method.attrs).unwrap_or_else(|| {
                        // Default minimal configuration if not specified
                        StateConfig {
                            states: vec![
                                StateDefinition {
                                    name: "initial".to_string(),
                                    deps: vec![],
                                },
                                StateDefinition {
                                    name: "authenticated".to_string(),
                                    deps: vec!["initial".to_string()],
                                },
                                StateDefinition {
                                    name: "authorized".to_string(),
                                    deps: vec!["authenticated".to_string()],
                                },
                            ],
                            transitions: vec![
                                TransitionDefinition {
                                    from: "initial".to_string(),
                                    to: "authenticated".to_string(),
                                    condition: None,
                                },
                                TransitionDefinition {
                                    from: "authenticated".to_string(),
                                    to: "authorized".to_string(),
                                    condition: None,
                                },
                            ],
                        }
                    });

                    // Add state check to the beginning of the method
                    let method_body = &method.block;
                    let access_control = quote! {
                        // Check if current state meets dependencies
                        self.state_manager.check_dependencies()?;
                    };

                    method.block = syn::parse2(quote! {
                        {
                            #access_control
                            #method_body
                        }
                    })
                    .unwrap();

                    // Remove the mcp_state attribute to avoid infinite recursion
                    method
                        .attrs
                        .retain(|attr| !attr.path().is_ident("mcp_state"));
                }
            }
        }

        TokenStream::from(quote! {
            #impl_item
        })
    } else {
        // If we can't parse as either function or impl, return the original
        item
    }
}

/// Procedural macro for defining RBAC rules for methods
#[allow(dead_code)]
pub fn impl_rbac_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(input) = syn::parse::<ItemFn>(item.clone()) {
        // Handle standalone function
        let method_name = &input.sig.ident;
        let args = &input.sig.inputs;
        let output = &input.sig.output;
        let block = &input.block;
        let visibility = &input.vis;

        // Parse attributes to extract role requirements
        let required_role = syn::parse_macro_input!(attr as LitStr).value();

        // Generate code with RBAC checks
        let expanded = quote! {
            #visibility fn #method_name(#args) #output {
                // Check if user has the required role before executing the method
                if let Some(current_user) = self.get_current_user() {
                    if !current_user.has_role(#required_role) {
                        return Err(::mcpr::error::MCPError::Authorization(
                            format!("Access denied: Role '{}' required", #required_role)
                        ));
                    }
                } else {
                    return Err(::mcpr::error::MCPError::Authentication(
                        "No authenticated user found".to_string()
                    ));
                }

                // If authorized, execute the original method
                #block
            }
        };

        TokenStream::from(expanded)
    } else if let Ok(mut impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        // Handle impl method
        // Look for methods with the #[requires_role] attribute and apply our transform
        for item in &mut impl_item.items {
            if let ImplItem::Fn(ref mut method) = item {
                // Check if the method has the requires_role attribute
                let has_rbac_attr = method
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("requires_role"));

                if has_rbac_attr {
                    // Extract role requirement
                    let mut required_role = "admin".to_string(); // Default
                    for attr in &method.attrs {
                        if attr.path().is_ident("requires_role") {
                            if let Ok(Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            })) = attr.meta.require_name_value().map(|v| v.value.clone())
                            {
                                required_role = s.value();
                            }
                        }
                    }

                    // Add RBAC check to the beginning of the method
                    let method_body = &method.block;
                    let required_role_val = required_role.clone();
                    let rbac_check = quote! {
                        // Check if user has the required role before executing the method
                        if let Some(current_user) = self.get_current_user() {
                            if !current_user.has_role(#required_role_val) {
                                return Err(::mcpr::error::MCPError::Authorization(
                                    format!("Access denied: Role '{}' required", #required_role_val)
                                ));
                            }
                        } else {
                            return Err(::mcpr::error::MCPError::Authentication(
                                "No authenticated user found".to_string()
                            ));
                        }
                    };

                    method.block = syn::parse2(quote! {
                        {
                            #rbac_check
                            #method_body
                        }
                    })
                    .unwrap();

                    // Remove the requires_role attribute to avoid infinite recursion
                    method
                        .attrs
                        .retain(|attr| !attr.path().is_ident("requires_role"));
                }
            }
        }

        TokenStream::from(quote! {
            #impl_item
        })
    } else {
        // If we can't parse as either function or impl, return the original
        item
    }
}
