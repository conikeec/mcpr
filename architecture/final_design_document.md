# MCPR Enhanced Framework Design Document

## Executive Summary

This document presents a comprehensive design for enhancing the mcpr Rust library with declarative style macros to simplify the creation of Model Context Protocol (MCP) hosts, clients, and servers. The design focuses on making the framework more intuitive and convenient for users while maintaining flexibility and adhering to the DRY (Don't Repeat Yourself) principle.

The enhanced framework will feature:
- Declarative attribute macros for defining MCP components
- YAML-based configuration with flexible transport options
- Support for mixed transport modes (SSE, WebSocket, stdio)
- Enhanced code generation with specification-based templates
- Cloud deployment support for major providers

This design builds upon the existing mcpr library while introducing significant improvements based on user feedback and industry best practices.

## Table of Contents

1. [Introduction](#introduction)
2. [Background and Research](#background-and-research)
3. [Design Goals](#design-goals)
4. [Macro Architecture](#macro-architecture)
5. [Configuration System](#configuration-system)
6. [Transport System](#transport-system)
7. [Generator Templates](#generator-templates)
8. [Implementation Strategy](#implementation-strategy)
9. [Compatibility and Migration](#compatibility-and-migration)
10. [Conclusion](#conclusion)

## Introduction

The Model Context Protocol (MCP) is an open standard developed by Anthropic for connecting AI assistants to data sources and tools. The current mcpr library provides a Rust implementation of MCP with features like schema definitions, transport options, client/server implementations, and project generation tools.

This design document outlines enhancements to the mcpr library that introduce a declarative macro-based approach, making it significantly easier to implement MCP components while maintaining the flexibility and power of the existing library.

## Background and Research

### Model Context Protocol

The Model Context Protocol is a standard for connecting AI assistants to the systems where data lives, including content repositories, business tools, and development environments. It provides a universal, open standard for connecting AI systems with data sources, replacing fragmented integrations with a single protocol.

Key components of MCP include:
- **Tools**: Functions that can be called by AI assistants
- **Prompts**: Parameterized text templates for AI assistants
- **Resources**: Content that can be accessed by AI assistants

MCP uses JSON-RPC as its communication protocol and supports multiple transport options, including HTTP and stdin/stdout.

### Existing mcpr Library

The current mcpr library provides a Rust implementation of MCP with:
- Schema definitions for the MCP protocol
- Transport layer options including stdio and SSE
- High-level client and server implementations
- CLI tools for generating server and client stubs
- Project generator for scaffolding new MCP projects

While functional, the current implementation requires significant boilerplate code and manual configuration, which users have requested to simplify.

### User Requirements

Based on user feedback and feature requests, the following requirements have been identified:
- Simplify implementation with declarative macros
- Support mixed transport options
- Improve configuration with YAML support
- Enhance deployment options for cloud environments
- Maintain backward compatibility

## Design Goals

The enhanced mcpr library aims to achieve the following goals:

1. **Simplify Implementation**: Reduce boilerplate code through declarative macros
2. **Enhance Flexibility**: Support mixed transport options and configuration
3. **Improve Maintainability**: Follow DRY principles for easier code maintenance
4. **Ensure Type Safety**: Leverage Rust's type system for better error detection
5. **Support Cloud Deployment**: Make it convenient to deploy in cloud environments
6. **Maintain Compatibility**: Allow gradual adoption alongside existing code

## Macro Architecture

The enhanced mcpr library will introduce a set of declarative attribute macros that transform implementation blocks into fully functional MCP components.

### Core Macros

#### `#[mcp_server]` Macro

Transforms an implementation block into a fully functional MCP server.

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

#### `#[mcp_client]` Macro

Transforms an implementation block into an MCP client.

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

#### `#[mcp_host]` Macro

Creates a host that can manage multiple servers and clients.

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

### Component Macros

#### `#[prompts]` Macro

Defines prompt methods within a server implementation.

```rust
#[prompts(
    transport = "stdio|sse|websocket",
    prefix = "optional_prefix"
)]
impl ServerStruct {
    /// Description of the prompt
    #[prompt(name = "prompt_name")]
    async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
        // Implementation
    }
}
```

#### `#[resources]` Macro

Defines resource methods within a server implementation.

```rust
#[resources(
    transport = "stdio|sse|websocket",
    base_uri = "optional_base_uri"
)]
impl ServerStruct {
    /// Description of the resource
    #[resource(uri = "resource_uri_template", mime_type = "text/plain")]
    async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
        // Implementation
    }
}
```

#### `#[tools]` Macro

Defines tool methods within a server implementation.

```rust
#[tools(
    transport = "stdio|sse|websocket",
    prefix = "optional_prefix"
)]
impl ServerStruct {
    /// Description of the tool
    #[tool(name = "tool_name")]
    async fn method_name(&self, param1: Type1, param2: Type2) -> Result<ReturnType> {
        // Implementation
    }
}
```

#### `#[auth]` Macro

Defines authentication and authorization methods.

```rust
#[auth(
    transport = "stdio|sse|websocket",
    scheme = "bearer|basic|custom"
)]
impl ServerStruct {
    #[authenticate]
    async fn authenticate(&self, credentials: Credentials) -> Result<User> {
        // Authentication logic
    }

    #[authorize(resource = "resource_type")]
    async fn authorize(&self, user: User, action: String, resource: String) -> Result<bool> {
        // Authorization logic
    }
}
```

### Macro Implementation Strategy

Each macro will be implemented using procedural macros that:

1. Parse the attribute arguments and associated implementation block
2. Extract documentation comments for descriptions
3. Generate the appropriate trait implementations
4. Handle YAML configuration loading and integration
5. Generate the necessary code for the specified transport(s)

## Configuration System

The configuration system will use YAML files to provide flexible configuration options for MCP components.

### YAML Configuration Structure

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

### Configuration Features

The configuration system will support:

1. **Environment Variable Substitution**: `${ENV_VAR}`
2. **Includes**: `!include other_config.yaml`
3. **Overrides**: Command-line arguments can override config values
4. **Validation**: Schema validation for configuration

## Transport System

The transport system will provide flexible options for communication between MCP components.

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

### Transport Types

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

### Mixed Transport Support

The system will support different transports for different components:

1. **Component-Level Transport**: Specified in macro attributes
2. **Method-Level Transport**: Overridden for specific methods
3. **Runtime Selection**: Based on client capabilities

## Generator Templates

The enhanced mcpr library will include an improved project generator with specification-based templates.

### Decision: Enhanced Generator Templates

The design preserves and enhances the existing generator templates approach rather than replacing it entirely. This decision is based on:

1. The existing approach provides a solid foundation that users are familiar with
2. Enhancing rather than replacing allows for backward compatibility
3. The template generation capability is valuable for quick project setup
4. The new macro-based approach can be integrated into the existing framework

### Specification-Based Generation

The enhanced generator will use a YAML-based specification format:

```yaml
# project_spec.yaml
name: "my-mcp-project"
version: "0.1.0"
description: "My MCP project"

components:
  - type: "server"
    name: "MyServer"
    features:
      - "prompts"
      - "resources"
      - "tools"
    transport: "stdio"
    
  - type: "client"
    name: "MyClient"
    transport: "sse"
    
  - type: "host"
    name: "MyHost"
    servers:
      - "MyServer"
    clients:
      - "MyClient"

deployment:
  type: "docker"
  provider: "aws"
  
structure:
  monorepo: true  # or false for separate crates
  
macros:
  enabled: true  # Use the new macro-based approach
```

### Template Engine

The generator will use a template engine with:

1. **Variable substitution**: Replace placeholders with values from the specification
2. **Conditional sections**: Include or exclude sections based on the specification
3. **Loops**: Generate repeated sections for multiple components
4. **Includes**: Import common template sections

### Command-Line Interface

```bash
mcpr generate [options]
  --spec SPEC_FILE       Path to specification YAML file
  --output DIR           Output directory
  --template-dir DIR     Custom template directory (optional)
  --force                Overwrite existing files
  --dry-run              Show what would be generated without writing files
```

## Implementation Strategy

The implementation of the enhanced mcpr library will be phased to ensure stability and backward compatibility.

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

### Cloud Provider Integration

Support for major cloud providers through YAML configuration:

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

The enhanced design for the mcpr library introduces a declarative macro-based approach that significantly simplifies the implementation of MCP components while maintaining flexibility and type safety. The design adheres to Rust best practices and provides a clear migration path for existing code.

Key benefits of this design include:

1. **Simplified Development**: Declarative macros reduce boilerplate code
2. **Flexible Configuration**: YAML-based configuration with environment variable support
3. **Mixed Transport**: Support for different transports for different components
4. **Cloud Ready**: Built-in support for containerization and cloud deployment
5. **Backward Compatible**: Gradual adoption path for existing code

The implementation will be phased to ensure stability and backward compatibility, with a focus on developer experience and cloud deployment support.

This design represents a significant enhancement to the mcpr library that will make it more accessible, flexible, and powerful for developers building MCP-based applications.
