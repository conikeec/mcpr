# MCPR Enhanced Macro Architecture Design

## Overview

This document outlines the design for enhancing the mcpr library with declarative style macros to simplify the creation of Model Context Protocol (MCP) hosts, clients, and servers. The goal is to make it more intuitive and convenient for users to implement MCP components while maintaining flexibility and adhering to the DRY (Don't Repeat Yourself) principle.

## Core Design Principles

1. **Declarative Approach**: Use attribute macros to define MCP components with minimal boilerplate code
2. **Configuration Integration**: Support YAML configuration files for flexible transport and other settings
3. **Transport Flexibility**: Allow mixed transport options (SSE, WebSocket, stdio) for different components
4. **Composition**: Enable easy composition of servers, clients, and resources
5. **Type Safety**: Leverage Rust's type system for better error detection and IDE support
6. **Documentation-Driven**: Use doc comments for descriptions and metadata

## Macro Architecture

### 1. `#[mcp_server]` Macro

The `#[mcp_server]` macro transforms an implementation block into a fully functional MCP server.

```rust
#[mcp_server(
    name = "My Server",
    version = "1.0.0",
    config = "path/to/config.yaml", // Optional
    transport = "stdio" // Default transport, can be overridden in config
)]
impl MyServer {
    // Server implementation
}
```

#### Configuration YAML Structure

```yaml
# server_config.yaml
name: "My Server"
version: "1.0.0"
transport:
  type: "stdio" # or "sse" or "websocket"
  # Transport-specific settings
  sse:
    port: 8080
    path: "/events"
  websocket:
    port: 8081
    path: "/ws"
```

### 2. `#[mcp_client]` Macro

The `#[mcp_client]` macro transforms an implementation block into an MCP client.

```rust
#[mcp_client(
    name = "My Client",
    config = "path/to/config.yaml", // Optional
    transport = "stdio" // Default transport, can be overridden in config
)]
impl MyClient {
    // Client implementation
}
```

#### Configuration YAML Structure

```yaml
# client_config.yaml
name: "My Client"
transport:
  type: "stdio" # or "sse" or "websocket"
  # Transport-specific settings
  sse:
    url: "http://localhost:8080/events"
  websocket:
    url: "ws://localhost:8081/ws"
servers:
  - name: "Server1"
    uri: "stdio://path/to/server"
  - name: "Server2"
    uri: "http://localhost:8080/events"
```

### 3. `#[mcp_host]` Macro

The `#[mcp_host]` macro creates a host that can manage multiple servers and clients.

```rust
#[mcp_host(
    name = "My Host",
    config = "path/to/config.yaml"
)]
impl MyHost {
    // Host implementation
}
```

#### Configuration YAML Structure

```yaml
# host_config.yaml
name: "My Host"
servers:
  - name: "Server1"
    type: "stdio"
    path: "./server1"
  - name: "Server2"
    type: "sse"
    port: 8080
clients:
  - name: "Client1"
    servers:
      - "Server1"
      - "Server2"
```

### 4. `#[prompts]` Macro

The `#[prompts]` macro defines prompt methods within a server implementation.

```rust
#[prompts(
    transport = "sse" // Optional override for transport
)]
impl MyServer {
    /// Description of the prompt
    #[prompt(name = "greeting")] // Name is optional, defaults to method name
    async fn greeting(&self, name: String) -> Result<String> {
        Ok(format!("Hello, {}!", name))
    }
}
```

### 5. `#[resources]` Macro

The `#[resources]` macro defines resource methods within a server implementation.

```rust
#[resources(
    transport = "websocket" // Optional override for transport
)]
impl MyServer {
    /// Description of the resource
    #[resource(uri = "my_app://files/{name}.txt")]
    async fn read_file(&self, name: String) -> Result<String> {
        // Implementation
        Ok(format!("Content of {}.txt", name))
    }
}
```

### 6. `#[tools]` Macro

The `#[tools]` macro defines tool methods within a server implementation.

```rust
#[tools(
    transport = "stdio" // Optional override for transport
)]
impl MyServer {
    /// Description of the tool
    #[tool(name = "add")] // Name is optional, defaults to method name
    async fn add(&self, a: i32, b: i32) -> Result<i32> {
        Ok(a + b)
    }
}
```

### 7. `#[auth]` Macro

The `#[auth]` macro defines authentication and authorization methods.

```rust
#[auth]
impl MyServer {
    #[authenticate]
    async fn authenticate(&self, token: String) -> Result<User> {
        // Authentication logic
        Ok(User { id: "user123".to_string() })
    }
    
    #[authorize(resource = "files")]
    async fn authorize_files(&self, user: &User, action: String) -> Result<bool> {
        // Authorization logic
        Ok(true)
    }
}
```

## Transport Configuration

The architecture supports specifying different transports for different components:

1. **Default Transport**: Specified at the server/client level
2. **Component-Specific Transport**: Overridden at the prompts/resources/tools level
3. **YAML Configuration**: Can override both of the above

This allows for flexible mixed transport configurations, such as:
- SSE for resources (better for streaming data)
- WebSocket for tools (better for bidirectional communication)
- stdio for local development and testing

## Macro Implementation Strategy

Each macro will be implemented using procedural macros that:

1. Parse the attribute arguments and associated implementation block
2. Extract documentation comments for descriptions
3. Generate the appropriate trait implementations
4. Handle YAML configuration loading and integration
5. Generate the necessary code for the specified transport(s)

The implementation will follow a similar pattern to the mcp-attr library but with enhanced features:

```rust
// Example implementation structure for #[mcp_server] macro
#[proc_macro_attribute]
pub fn mcp_server(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse attributes
    let attrs = parse_macro_input!(attr as ServerAttr);
    
    // Parse implementation block
    let input = parse_macro_input!(item as ItemImpl);
    
    // Extract server name, version, etc.
    let server_name = attrs.name.unwrap_or_else(|| /* default */);
    
    // Load YAML config if specified
    let config = if let Some(path) = attrs.config {
        quote! {
            let config = ::mcpr::config::load_yaml(#path)?;
        }
    } else {
        quote! {}
    };
    
    // Generate implementation
    let expanded = quote! {
        // Generated code
    };
    
    TokenStream::from(expanded)
}
```

## Code Generation

The macros will generate:

1. **Trait Implementations**: For the MCP protocol interfaces
2. **Transport Handlers**: Based on the specified transport(s)
3. **Configuration Loading**: For YAML integration
4. **Error Handling**: With appropriate context and messages
5. **Documentation**: Preserving doc comments in generated code

## Integration with Existing Code

The enhanced macros will be designed to be compatible with the existing mcpr library, allowing for gradual adoption:

```rust
// Using the enhanced macros
#[mcp_server]
impl MyServer {
    // ...
}

// Equivalent manual implementation
let server_config = ServerConfig::new()
    .with_name("My Server")
    .with_version("1.0.0");

let mut server = Server::new(server_config);
// ... register handlers manually
```

## Template Generation

The existing template generation functionality will be preserved but enhanced to support the new macro-based approach:

```bash
# Generate a project with the new macro-based approach
mcpr generate-project --name my-project --transport stdio --use-macros
```

The generated project will include examples of using the new macros, with comments explaining the available options.
