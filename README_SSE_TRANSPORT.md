# SSE Transport for MCP Server SDK

This document explains the Server-Sent Events (SSE) transport implementation for the MCP (Model Context Protocol) server SDK.

## Overview

The SSE transport enables server-to-client communication over HTTP using the Server-Sent Events protocol. This implementation follows the MCP specification for SSE-based communication.

## How It Works

### Protocol Flow (MCP Compliant)

1. **Connection Establishment**:

   - Clients establish HTTP connections to the server's SSE endpoint
   - Server responds with `Content-Type: text/event-stream`
   - Connection remains open for server to push events

2. **Endpoint Discovery**:

   - As per MCP requirements, the server sends an initial `endpoint` event containing the URI for clients to send messages
   - Format: `event: endpoint\ndata: http://server-address/messages\n\n`
   - Clients must use this provided URI for all message requests

3. **Message Format**:

   - Server messages are sent as SSE events with the format:

     ```
     event: message
     data: {"jsonrpc":"2.0",...}

     ```

   - The `event` field identifies the type of message (`endpoint` or `message`)
   - The `data` field contains the JSON-serialized MCP message

4. **Receiving Messages**:
   - Clients send messages to the server via HTTP POST to the endpoint provided in the initial endpoint event
   - Server processes these messages and responds via the SSE stream

### Implementation Details

The SSE transport implementation consists of:

1. **`SSETransport` Struct**:
   - Implements the `Transport` trait for use with the MCP Server
   - Manages an HTTP server for SSE event streaming and message reception
   - Handles message serialization/deserialization
   - Provides message broadcasting capability
   - Fully compliant with MCP protocol requirements

## Usage with MCP Server

The SSE transport is designed specifically for server-side use with the MCP Server implementation:

```rust
use mcpr::{
    error::MCPError,
    schema::common::{Tool, ToolInputSchema},
    server::{Server, ServerConfig},
    transport::sse::SSETransport,
};

#[tokio::main]
async fn main() -> Result<(), MCPError> {
    // Create a transport for SSE server
    let uri = "http://127.0.0.1:8000";
    let transport = SSETransport::new_server(uri)?;

    // Configure the server with tools
    let server_config = ServerConfig::new()
        .with_name("SSE MCP Server")
        .with_version("1.0.0")
        .with_tool(/* your tool definition */);

    // Create the server
    let mut server = Server::new(server_config);

    // Register tool handlers
    server.register_tool_handler("your_tool", |params| async move {
        // Handle tool call
        Ok(serde_json::json!({ "result": "Success" }))
    })?;

    // Start the server with SSE transport
    server.start_background(transport).await?;

    // Server is now running and accessible at:
    // - SSE endpoint: http://127.0.0.1:8000/events
    // - Message endpoint: http://127.0.0.1:8000/messages

    // Wait for shutdown signal...

    Ok(())
}
```

### Running the Example

Run the MCP server with SSE transport:

```
cargo run --example sse_mcp_server
```

This will start a server accessible at:

- SSE endpoint: http://127.0.0.1:8000/events
- Message endpoint: http://127.0.0.1:8000/messages

## Client Connection Protocol

When a client connects to the SSE endpoint:

1. The server sends an "endpoint" event with the URI where the client should send messages:

   ```
   event: endpoint
   data: http://server.example.com/messages

   ```

2. All subsequent server-to-client messages are sent as "message" events:

   ```
   event: message
   data: {"jsonrpc":"2.0","id":1,"result":{"status":"success"}}

   ```

3. Clients must send their messages as HTTP POST requests to the endpoint URL provided in the initial event.

## Advantages of SSE Transport for MCP Servers

- Easily accessible over standard HTTP
- Works through most firewalls and proxies (standard HTTP)
- Compatible with many client environments (browsers, command line tools, etc.)
- Lightweight protocol with minimal overhead
- Automatic reconnection support in many client libraries
- Full compliance with MCP protocol specification

## Troubleshooting

### Server Configuration Issues

If clients can't connect to your SSE server:

1. **Endpoint paths**: Make sure you're using the correct endpoint paths

   - Default is `/events` for SSE streaming and `/messages` for message submission
   - These paths are fixed in the current implementation

2. **Network access**: Ensure clients can reach the server

   - By default, the server binds to 127.0.0.1, which is only accessible locally
   - To allow external connections, use a hostname or IP that is accessible

3. **Port conflicts**: If the server fails to start, check if another service is using the port
   - Default port is 8000, but can be specified in the URL

### Testing the Server

To check if your server is running correctly, you can test it with curl:

```bash
# Test the SSE endpoint
curl -N -H "Accept: text/event-stream" http://localhost:8000/events

# Send a message to the message endpoint
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' \
  http://localhost:8000/messages
```
