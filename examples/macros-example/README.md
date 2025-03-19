# MCPR Macros Example

This example demonstrates how to use the MCPR macros to create MCP clients and servers with minimal boilerplate.

## Overview

The example project consists of two parts:

1. **Server**: A simple MCP server that handles tool calls, implements prompt templates, and provides resources.
2. **Client**: A client that connects to the server and invokes the tools.

## Macros Used

This example demonstrates the use of the following MCPR macros that significantly reduce boilerplate:

- `#[mcp_transport(type)]`: Creates a transport implementation with zero boilerplate. The type parameter (e.g., "stdio") selects the transport type.
- `#[mcp_server]`: Creates a server with builder pattern support, automatically handling request processing.
- `#[mcp_client]`: Creates a client with builder pattern support, automatically handling connection management.
- `#[mcp_prompt]`: Defines a prompt provider with minimal code.
- `#[mcp_resource]`: Defines a resource provider with minimal code.
- `#[prompt]`: Marks a function as a prompt template, handling parameter serialization.
- `#[resource]`: Marks a function as a resource provider, handling parameter serialization.
- `#[tool]`: Marks a function as a tool on the server side, handling parameter deserialization.
- `#[tool_call(name)]`: Defines a client-side tool call method that maps to a server-side tool.

## Key Benefits

- **Drastically Reduced Boilerplate**: The macros handle all the repetitive code like serialization, deserialization, error handling, and transport management.
- **Builder Pattern**: Both client and server use a builder pattern for configuration.
- **Type Safety**: The tool and resource macros provide type-safe interfaces.
- **Simplified Transport**: Transport creation and management is abstracted away.

## Building Instructions

To build the project:

```
# Build both client and server
cargo build

# Or build separately
cd server
cargo build
cd ../client
cargo build
```

## Running the Example

You can run the example in two ways:

### Method 1: Let client start the server

```
cd client
cargo run
```

The client will automatically start the server process.

### Method 2: Run server and client separately

In one terminal:
```
cd server
cargo run
```

In another terminal:
```
cd client
cargo run -- --connect
```

## Understanding the Code

### Server

The server code shows how to:

1. Define a custom transport using `#[mcp_transport(stdio)]`
2. Create a prompt provider with `#[mcp_prompt]` and `#[prompt]`
3. Create a resource provider with `#[mcp_resource]` and `#[resource]`
4. Define a server with `#[mcp_server]` and tools using `#[tool]`
5. Use the builder pattern to set up and start the server

### Client

The client code demonstrates:

1. Creating a transport with `#[mcp_transport(stdio)]`
2. Defining a client with `#[mcp_client]`
3. Implementing tool call methods using `#[tool_call(name)]`
4. Using the builder pattern to configure and connect the client

The macros handle all JSON-RPC communication, parameter serialization/deserialization, and error handling. 