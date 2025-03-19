extern crate proc_macro;

use proc_macro::TokenStream;

mod auth;
mod client;
mod prompt;
mod resource;
mod server;
mod state;
mod tool;
mod tools;
mod transport;

/// Attribute macro for implementing a transport type with all required methods
///
/// # Example
///
/// ```
/// #[mcp_transport(stdio)]
/// pub struct MyTransport {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_transport(attr: TokenStream, item: TokenStream) -> TokenStream {
    transport::impl_transport_macro(attr, item)
}

/// Attribute macro for implementing a client with builder pattern and all required methods
///
/// # Example
///
/// ```
/// #[mcp_client]
/// pub struct MyClient {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_client(attr: TokenStream, item: TokenStream) -> TokenStream {
    client::impl_client_macro(attr, item)
}

/// Attribute macro for tool call methods on a client
///
/// # Example
///
/// ```
/// #[tool_call("hello")]
/// pub fn say_hello(&mut self, name: String) -> Result<String, MCPError> {
///     self.call_tool("hello", serde_json::json!({ "name": name }))
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    client::tool_call_attr_macro(attr, item)
}

/// Attribute macro for implementing a server with builder pattern and all required methods
///
/// # Example
///
/// ```
/// #[mcp_server]
/// pub struct MyServer {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_server(attr: TokenStream, item: TokenStream) -> TokenStream {
    server::impl_server_macro(attr, item)
}

/// Attribute macro for tool methods on a server
///
/// # Example
///
/// ```
/// #[tool("hello")]
/// pub fn say_hello(&self, name: String) -> Result<String, MCPError> {
///     Ok(format!("Hello, {}!", name))
/// }
/// ```
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::impl_tool_attr_macro(attr, item)
}

/// Attribute macro for implementing a prompt provider
///
/// # Example
///
/// ```
/// #[mcp_prompt]
/// pub struct MyPromptProvider {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_prompt(attr: TokenStream, item: TokenStream) -> TokenStream {
    prompt::impl_prompt_macro(attr, item)
}

/// Attribute macro for prompt methods
///
/// # Example
///
/// ```
/// #[prompt]
/// pub fn system_prompt(&self) -> String {
///     "You are a helpful assistant".to_string()
/// }
/// ```
#[proc_macro_attribute]
pub fn prompt(attr: TokenStream, item: TokenStream) -> TokenStream {
    prompt::prompt_attr_macro(attr, item)
}

/// Attribute macro for implementing a resource provider
///
/// # Example
///
/// ```
/// #[mcp_resource]
/// pub struct MyResourceProvider {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::impl_resource_macro(attr, item)
}

/// Attribute macro for resource methods
///
/// # Example
///
/// ```
/// #[resource]
/// pub fn get_user_info(&self) -> serde_json::Value {
///     serde_json::json!({ "name": "Example User" })
/// }
/// ```
#[proc_macro_attribute]
pub fn resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::resource_attr_macro(attr, item)
}

/// Attribute macro for implementing a tools provider
///
/// # Example
///
/// ```
/// #[mcp_tools]
/// pub struct MyToolsProvider {
///     // Fields will be auto-generated
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_tools(attr: TokenStream, item: TokenStream) -> TokenStream {
    tools::impl_tools_macro(attr, item)
}

/// Attribute macro for tool definitions with metadata
///
/// # Example
///
/// ```
/// #[tool_def("Calculate the sum of two numbers")]
/// pub fn add(&self, a: i32, b: i32) -> Result<i32, MCPError> {
///     Ok(a + b)
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_def(attr: TokenStream, item: TokenStream) -> TokenStream {
    tools::tool_attr_macro(attr, item)
}

// Authentication macros (placeholder for future use)
#[proc_macro_attribute]
pub fn mcp_auth(attr: TokenStream, item: TokenStream) -> TokenStream {
    auth::impl_auth_macro(attr, item)
}

// State management macros (placeholder for future use)
#[proc_macro_attribute]
pub fn mcp_state(attr: TokenStream, item: TokenStream) -> TokenStream {
    state::impl_state_macro(attr, item)
}
