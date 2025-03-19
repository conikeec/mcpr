# MCPR Enhanced Design Specification

## Introduction

This document provides a detailed design specification for enhancing the mcpr library with declarative style macros. The goal is to create a more intuitive and convenient framework for implementing Model Context Protocol (MCP) hosts, clients, and servers in Rust, while maintaining flexibility and adhering to the DRY principle.

## Background

The Model Context Protocol (MCP) is an open standard developed by Anthropic for connecting AI assistants to data sources and tools. The current mcpr library provides a Rust implementation of MCP with features like schema definitions, transport options, client/server implementations, and project generation tools. However, users have requested a more declarative approach using macros to simplify development.

## Design Goals

1. **Simplify Implementation**: Reduce boilerplate code through declarative macros
2. **Enhance Flexibility**: Support mixed transport options and configuration
3. **Improve Maintainability**: Follow DRY principles for easier code maintenance
4. **Ensure Type Safety**: Leverage Rust's type system for better error detection
5. **Support Cloud Deployment**: Make it convenient to deploy in cloud environments
6. **Maintain Compatibility**: Allow gradual adoption alongside existing code

## Macro System Architecture

### Core Components

The enhanced mcpr library will consist of the following core components:

1. **Procedural Macros**: A set of attribute macros for declarative MCP implementation
2. **Configuration System**: YAML-based configuration with runtime loading
3. **Transport Layer**: Flexible transport options with mixed-mode support
4. **Code Generation**: Template generation for quick project scaffolding
5. **Runtime Library**: Supporting types and functions for the generated code

### Dependency Structure

```
mcpr (main crate)
├── mcpr-macros (proc-macro crate)
│   ├── mcp_server_macro
│   ├── mcp_client_macro
│   ├── mcp_host_macro
│   ├── prompts_macro
│   ├── resources_macro
│   ├── tools_macro
│   └── auth_macro
├── transport
│   ├── stdio
│   ├── sse
│   └── websocket
├── config
│   └── yaml
└── generator
    └── templates
```

## Detailed Macro Specifications

### 1. `#[mcp_server]` Macro

#### Purpose
Transform an implementation block into a fully functional MCP server.

#### Syntax
```rust
#[mcp_server(
    name = "Server Name",
    version = "1.0.0",
    config = "path/to/config.yaml",
    transport = "stdio|sse|websocket",
    description = "Optional server description"
)]
impl ServerStruct {
    // Server implementation
}
```

#### Parameters
- `name` (optional): Server name, defaults to struct name
- `version` (optional): Server version, defaults to "0.1.0"
- `config` (optional): Path to YAML configuration file
- `transport` (optional): Default transport type, defaults to "stdio"
- `description` (optional): Server description, can also use doc comments

#### Generated Code
The macro will generate:
1. Implementation of the `McpServer` trait
2. Transport setup based on configuration
3. Capability declarations based on implemented methods
4. Error handling and logging infrastructure

#### Example
```rust
/// My MCP server for file operations
#[mcp_server(
    name = "FileServer",
    version = "1.0.0",
    config = "server_config.yaml"
)]
impl FileServer {
    // Server state and implementation
}
```

### 2. `#[mcp_client]` Macro

#### Purpose
Transform an implementation block into an MCP client.

#### Syntax
```rust
#[mcp_client(
    name = "Client Name",
    config = "path/to/config.yaml",
    transport = "stdio|sse|websocket",
    servers = ["server1", "server2"]
)]
impl ClientStruct {
    // Client implementation
}
```

#### Parameters
- `name` (optional): Client name, defaults to struct name
- `config` (optional): Path to YAML configuration file
- `transport` (optional): Default transport type, defaults to "stdio"
- `servers` (optional): List of server names to connect to

#### Generated Code
The macro will generate:
1. Implementation of the `McpClient` trait
2. Transport setup based on configuration
3. Server connection management
4. Error handling and reconnection logic

#### Example
```rust
#[mcp_client(
    name = "FileClient",
    config = "client_config.yaml"
)]
impl FileClient {
    // Client implementation
}
```

### 3. `#[mcp_host]` Macro

#### Purpose
Create a host that can manage multiple servers and clients.

#### Syntax
```rust
#[mcp_host(
    name = "Host Name",
    config = "path/to/config.yaml",
    servers = ["server1", "server2"],
    clients = ["client1", "client2"]
)]
impl HostStruct {
    // Host implementation
}
```

#### Parameters
- `name` (optional): Host name, defaults to struct name
- `config` (optional): Path to YAML configuration file
- `servers` (optional): List of server names to host
- `clients` (optional): List of client names to manage

#### Generated Code
The macro will generate:
1. Implementation of the `McpHost` trait
2. Server and client instantiation and management
3. Lifecycle management (start, stop, restart)
4. Error handling and status reporting

#### Example
```rust
#[mcp_host(
    name = "FileSystemHost",
    config = "host_config.yaml"
)]
impl FileSystemHost {
    // Host implementation
}
```

### 4. `#[prompts]` Macro

#### Purpose
Define prompt methods within a server implementation.

#### Syntax
```rust
#[prompts(
    transport = "stdio|sse|websocket",
    prefix = "optional_prefix"
)]
impl ServerStruct {
    // Prompt methods
}
```

#### Parameters
- `transport` (optional): Override transport for prompts
- `prefix` (optional): Prefix for prompt names

#### Child Attribute: `#[prompt]`
```rust
/// Description of the prompt
#[prompt(
    name = "prompt_name",
    description = "Prompt description"
)]
async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
    // Implementation
}
```

#### Generated Code
The macro will generate:
1. Implementation of prompt-related MCP methods
2. Parameter schema generation from Rust types
3. Documentation extraction from comments

#### Example
```rust
#[prompts]
impl FileServer {
    /// Returns a greeting message
    #[prompt(name = "greeting")]
    async fn get_greeting(&self, name: String) -> Result<String> {
        Ok(format!("Hello, {}!", name))
    }
}
```

### 5. `#[resources]` Macro

#### Purpose
Define resource methods within a server implementation.

#### Syntax
```rust
#[resources(
    transport = "stdio|sse|websocket",
    base_uri = "optional_base_uri"
)]
impl ServerStruct {
    // Resource methods
}
```

#### Parameters
- `transport` (optional): Override transport for resources
- `base_uri` (optional): Base URI for all resources

#### Child Attribute: `#[resource]`
```rust
/// Description of the resource
#[resource(
    uri = "resource_uri_template",
    mime_type = "text/plain"
)]
async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
    // Implementation
}
```

#### Generated Code
The macro will generate:
1. Implementation of resource-related MCP methods
2. URI template parsing and parameter extraction
3. MIME type handling

#### Example
```rust
#[resources(base_uri = "file://")]
impl FileServer {
    /// Reads a file from the server
    #[resource(uri = "{path}", mime_type = "text/plain")]
    async fn read_file(&self, path: String) -> Result<String> {
        // Implementation
    }
}
```

### 6. `#[tools]` Macro

#### Purpose
Define tool methods within a server implementation.

#### Syntax
```rust
#[tools(
    transport = "stdio|sse|websocket",
    prefix = "optional_prefix"
)]
impl ServerStruct {
    // Tool methods
}
```

#### Parameters
- `transport` (optional): Override transport for tools
- `prefix` (optional): Prefix for tool names

#### Child Attribute: `#[tool]`
```rust
/// Description of the tool
#[tool(
    name = "tool_name",
    description = "Tool description"
)]
async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
    // Implementation
}
```

#### Generated Code
The macro will generate:
1. Implementation of tool-related MCP methods
2. Parameter schema generation from Rust types
3. Documentation extraction from comments

#### Example
```rust
#[tools]
impl FileServer {
    /// Creates a new file
    #[tool(name = "create_file")]
    async fn create_file(&self, path: String, content: String) -> Result<bool> {
        // Implementation
        Ok(true)
    }
}
```

### 7. `#[auth]` Macro

#### Purpose
Define authentication and authorization methods.

#### Syntax
```rust
#[auth(
    transport = "stdio|sse|websocket",
    scheme = "bearer|basic|custom"
)]
impl ServerStruct {
    // Auth methods
}
```

#### Parameters
- `transport` (optional): Override transport for auth
- `scheme` (optional): Authentication scheme

#### Child Attributes: `#[authenticate]` and `#[authorize]`
```rust
#[authenticate]
async fn authenticate(&self, credentials: Credentials) -> Result<User> {
    // Authentication logic
}

#[authorize(resource = "resource_type")]
async fn authorize(&self, user: User, action: String, resource: String) -> Result<bool> {
    // Authorization logic
}
```

#### Generated Code
The macro will generate:
1. Authentication middleware
2. Authorization checks for resources and tools
3. Integration with transport security

#### Example
```rust
#[auth(scheme = "bearer")]
impl FileServer {
    #[authenticate]
    async fn authenticate(&self, token: String) -> Result<User> {
        // Validate token
        Ok(User { id: "user123".to_string() })
    }
    
    #[authorize(resource = "files")]
    async fn authorize_files(&self, user: User, action: String, path: String) -> Result<bool> {
        // Check permissions
        Ok(true)
    }
}
```

## Configuration System

### YAML Configuration Structure

The configuration system will use YAML files with the following structure:

```yaml
# Common configuration
name: "Service Name"
version: "1.0.0"
description: "Service description"

# Transport configuration
transport:
  default: "stdio"  # Default transport
  
  # Transport-specific configurations
  stdio:
    command: "./server"
    
  sse:
    port: 8080
    path: "/events"
    cors: true
    
  websocket:
    port: 8081
    path: "/ws"
    
# Server-specific configuration
server:
  capabilities:
    prompts: true
    resources: true
    tools: true
  
# Client-specific configuration
client:
  servers:
    - name: "Server1"
      uri: "stdio://./server1"
    - name: "Server2"
      uri: "http://localhost:8080/events"
      
# Host-specific configuration
host:
  servers:
    - name: "Server1"
      type: "stdio"
      path: "./server1"
    - name: "Server2"
      type: "sse"
      port: 8080
  clients:
    - name: "Client1"
      servers: ["Server1", "Server2"]
```

### Configuration Loading

The configuration will be loaded at runtime with support for:

1. **Environment Variable Substitution**: `${ENV_VAR}`
2. **Includes**: `!include other_config.yaml`
3. **Overrides**: Command-line arguments can override config values
4. **Validation**: Schema validation for configuration

## Transport System

### Transport Interface

All transports will implement a common interface:

```rust
pub trait Transport: Send + Sync {
    async fn initialize(&mut self) -> Result<()>;
    async fn send_message(&self, message: Message) -> Result<()>;
    async fn receive_message(&self) -> Result<Message>;
    async fn shutdown(&self) -> Result<()>;
}
```

### Mixed Transport Support

The system will support different transports for different components:

1. **Component-Level Transport**: Specified in macro attributes
2. **Method-Level Transport**: Overridden for specific methods
3. **Runtime Selection**: Based on client capabilities

### Transport Implementation Details

#### stdio Transport
- Uses standard input/output for communication
- Supports command execution for server processes
- Handles serialization/deserialization of JSON-RPC messages

#### SSE Transport
- Server-Sent Events over HTTP
- Long-polling for client-to-server communication
- Support for reconnection and backoff strategies

#### WebSocket Transport
- Bidirectional communication over WebSocket protocol
- Support for binary and text messages
- Heartbeat mechanism for connection health

## Code Generation and Templates

### Template Generation

The existing template generation will be enhanced:

```bash
mcpr generate-project [options]
  --name NAME             Project name
  --transport TRANSPORT   Transport type (stdio, sse, websocket)
  --use-macros            Use the new macro-based approach
  --components COMPONENTS Components to generate (server, client, host)
  --output DIR            Output directory
```

### Generated Project Structure

```
my-project/
├── Cargo.toml
├── config/
│   ├── server.yaml
│   ├── client.yaml
│   └── host.yaml
├── src/
│   ├── main.rs
│   ├── server.rs
│   ├── client.rs
│   └── host.rs
├── tests/
│   └── integration_tests.rs
└── README.md
```

### Example Generated Code

```rust
// server.rs
use mcpr::prelude::*;

pub struct MyServer {
    // Server state
}

#[mcp_server(
    name = "MyServer",
    config = "config/server.yaml"
)]
impl MyServer {
    pub fn new() -> Self {
        Self { /* initialize state */ }
    }
}

#[prompts]
impl MyServer {
    /// Example prompt
    #[prompt]
    async fn hello(&self, name: String) -> Result<String> {
        Ok(format!("Hello, {}!", name))
    }
}

#[tools]
impl MyServer {
    /// Example tool
    #[tool]
    async fn add(&self, a: i32, b: i32) -> Result<i32> {
        Ok(a + b)
    }
}
```

## Implementation Strategy

### Phase 1: Core Macro Framework
1. Implement the procedural macro crate structure
2. Develop the basic attribute parsing
3. Create the code generation templates

### Phase 2: Transport System
1. Refactor existing transports to support the new architecture
2. Implement the mixed transport capability
3. Add the WebSocket transport

### Phase 3: Configuration System
1. Implement YAML configuration loading
2. Add environment variable substitution
3. Create configuration validation

### Phase 4: Template Generation
1. Update the project generator
2. Create templates for the new macro-based approach
3. Add examples and documentation

## Compatibility and Migration

### Backward Compatibility

The enhanced library will maintain compatibility with existing code:

```rust
// Old approach
let server_config = ServerConfig::new()
    .with_name("My Server")
    .with_version("1.0.0");

let mut server = Server::new(server_config);
server.register_tool_handler("add", |params| {
    // Handler implementation
})?;

// New approach
#[mcp_server(name = "My Server", version = "1.0.0")]
impl MyServer {
    #[tool(name = "add")]
    async fn add(&self, a: i32, b: i32) -> Result<i32> {
        Ok(a + b)
    }
}
```

### Migration Path

1. **Gradual Adoption**: Use macros for new components while keeping existing code
2. **Mixed Usage**: Combine macro-based and manual approaches
3. **Migration Tool**: Provide a tool to convert existing code to the new approach

## Cloud Deployment Support

### Containerization

Support for containerized deployment:

```rust
#[mcp_server(
    name = "MyServer",
    config = "config/server.yaml",
    deployment = "docker"
)]
impl MyServer {
    // Implementation
}
```

Generated Dockerfile:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/my-server /usr/local/bin/
COPY config /etc/my-server/
CMD ["my-server"]
```

### Cloud Provider Integration

Support for major cloud providers:

```yaml
# deployment.yaml
provider: "aws"  # or "gcp", "azure"
service_type: "lambda"  # or "cloud_run", "azure_functions"
region: "us-west-2"
memory: 512
timeout: 30
environment:
  RUST_LOG: "info"
```

## Conclusion

This enhanced design for the mcpr library introduces a declarative macro-based approach that significantly simplifies the implementation of MCP components while maintaining flexibility and type safety. The design adheres to Rust best practices and provides a clear migration path for existing code.

The implementation will be phased to ensure stability and backward compatibility, with a focus on developer experience and cloud deployment support.
