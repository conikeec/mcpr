# MCPR Macros

Procedural macros for the MCPR (Model Context Protocol in Rust) implementation.

## Overview

This crate provides procedural macros that help reduce boilerplate when implementing MCP components. The macros generate common code patterns while allowing for customization where needed.

## Available Macros

- `#[mcp_transport]`: For implementing the Transport trait with minimal boilerplate (IMPLEMENTED)
- `#[mcp_server]`: For implementing MCP servers (IN PROGRESS)
- `#[tool]`: For defining tools in MCP servers (IN PROGRESS)
- `#[mcp_client]`: For implementing MCP clients (PLANNED)
- `#[prompt]`: For defining prompts in MCP servers (PLANNED)
- `#[resource]`: For defining resources in MCP servers (PLANNED)
- `#[auth]`: For defining authentication requirements (PLANNED)

## Usage

Add the `macros` feature to your MCPR dependency:

```toml
[dependencies]
mcpr = { version = "0.2.3", features = ["macros"] }
```

### Transport Macro

To use the transport macro, apply it to a struct that implements the Transport trait:

```rust
use mcpr::macros::mcp_transport;
use mcpr::error::MCPError;
use mcpr::transport::{CloseCallback, ErrorCallback, Transport};

#[mcp_transport]
struct MyTransport {
    is_connected: bool,
    on_close: Option<CloseCallback>,
    on_error: Option<ErrorCallback>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
    // Transport-specific fields
}

// You'll need to implement transport-specific methods like send and receive
```

### Server Macro (Work in Progress)

The server macro will generate code for MCP servers with minimal boilerplate:

```rust
use mcpr::macros::{mcp_server, tool};
use mcpr::server::McpServer;

struct MyServer;

#[mcp_server]
impl McpServer for MyServer {
    #[tool]
    async fn my_tool(&self, param: String) -> Result<String, MCPError> {
        // Tool implementation
        Ok(format!("Processed: {}", param))
    }
}
```

## Schema Alignment

The macros are designed to align with the MCP schema definitions in the `schema` module:

- **Transport Macros**: Implements the Transport trait for communication
- **Server Macros**: Implements server-side methods defined in `schema/server.rs`
- **Client Macros**: Will implement client methods defined in `schema/client.rs`
- **Method Attributes**: Will generate code that conforms to the appropriate schema structures

## Next Steps

1. **Transport Layer (Complete)**:
   - ✅ Basic implementation of the `#[mcp_transport]` macro
   - ✅ Support for common Transport trait methods

2. **Server Layer (In Progress)**:
   - ✅ Basic structure for `#[mcp_server]` macro
   - 🔄 Support for `#[tool]` attribute
   - ⬜️ Proper handling of parameter schemas
   - ⬜️ Implement tools_call to handle invocation
   - ⬜️ Support for async methods

3. **Client Layer (Pending)**:
   - ⬜️ Implement `#[mcp_client]` macro
   - ⬜️ Support for auto-generating request methods

4. **Method Attributes (Planned)**:
   - ⬜️ Complete `#[tool]` attribute implementation
   - ⬜️ Implement `#[prompt]` attribute
   - ⬜️ Implement `#[resource]` attribute

5. **Authentication (Planned)**:
   - ⬜️ Implement `#[auth]` attribute
   - ⬜️ Support for different authentication schemes

6. **Testing and Documentation**:
   - ✅ Basic unit tests
   - 🔄 Integration tests
   - ⬜️ Comprehensive documentation
   - ⬜️ Usage examples

## Development Guidelines

- Minimize changes to existing code outside the macro crate
- Use feature flags to enable/disable macro functionality
- Write comprehensive tests for each macro
- Ensure generated code is compatible with existing interfaces 