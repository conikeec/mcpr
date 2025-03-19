# Macro Dependency Matrix for MCPR Framework

This document provides a comprehensive dependency matrix showing the relationships between the core macros (`#[mcp_host]`, `#[mcp_client]`, `#[mcp_server]`) and the component macros (`#[tools]`, `#[resources]`, `#[prompts]`, `#[auth]`) in the MCPR framework, as well as how each macro interacts with the state machine validation system.

## Core Macro Dependency Matrix

| Macro | Depends On | Required By | State Machine Interactions |
|-------|------------|-------------|----------------------------|
| `#[mcp_server]` | None | `#[mcp_client]`, `#[mcp_host]`, `#[tools]`, `#[resources]`, `#[prompts]`, `#[auth]` | • Development: Base validation<br>• Build: Component registration<br>• Deployment: Service configuration<br>• Runtime: Service lifecycle |
| `#[mcp_client]` | `#[mcp_server]` | `#[mcp_host]` | • Development: Server dependency validation<br>• Build: Transport compatibility check<br>• Deployment: Connection configuration<br>• Runtime: Auto-refresh on server changes |
| `#[mcp_host]` | `#[mcp_server]`, `#[mcp_client]` | None | • Development: Component dependency validation<br>• Build: Orchestration validation<br>• Deployment: Environment configuration<br>• Runtime: Component lifecycle management |

## Component Macro Dependency Matrix

| Macro | Depends On | Required By | State Machine Interactions |
|-------|------------|-------------|----------------------------|
| `#[tools]` | `#[mcp_server]` | None | • Development: Server association validation<br>• Build: Parameter type checking<br>• Deployment: Tool capability registration<br>• Runtime: Auto-refresh client tools |
| `#[resources]` | `#[mcp_server]` | None | • Development: Server association validation<br>• Build: URI template validation<br>• Deployment: Resource registration<br>• Runtime: Resource caching management |
| `#[prompts]` | `#[mcp_server]` | None | • Development: Server association validation<br>• Build: Template syntax validation<br>• Deployment: Prompt registration<br>• Runtime: Template caching management |
| `#[auth]` | `#[mcp_server]` | None | • Development: Protected component validation<br>• Build: Auth handler validation<br>• Deployment: Security configuration<br>• Runtime: Auth token management |

## Cross-Component Relationships

| Macro | Relationship with Tools | Relationship with Resources | Relationship with Prompts | Relationship with Auth |
|-------|-------------------------|----------------------------|--------------------------|------------------------|
| `#[mcp_server]` | • Registers tools<br>• Exposes tool endpoints | • Registers resources<br>• Serves resource content | • Registers prompts<br>• Renders prompt templates | • Applies auth middleware<br>• Enforces access control |
| `#[mcp_client]` | • Discovers tools<br>• Invokes tool methods<br>• Auto-refreshes on changes | • Discovers resources<br>• Fetches resource content<br>• Caches resources | • Discovers prompts<br>• Renders prompt templates<br>• Caches rendered prompts | • Provides auth credentials<br>• Manages auth tokens |
| `#[mcp_host]` | • Orchestrates tool access<br>• Routes tool requests | • Orchestrates resource access<br>• Manages resource caching | • Orchestrates prompt access<br>• Manages prompt caching | • Centralizes auth configuration<br>• Manages auth state |
| `#[tools]` | • (Self-referential) | • Can access resources<br>• Can modify resources | • Can use prompts<br>• Can generate prompts | • Protected by auth<br>• Can perform privileged actions |
| `#[resources]` | • Can be modified by tools<br>• Can trigger tool execution | • (Self-referential) | • Can contain prompt templates<br>• Can be used in prompts | • Protected by auth<br>• Can have access levels |
| `#[prompts]` | • Can invoke tools<br>• Can reference tool results | • Can include resources<br>• Can reference resource content | • (Self-referential) | • Protected by auth<br>• Can have access levels |
| `#[auth]` | • Protects tool access<br>• Validates tool permissions | • Protects resource access<br>• Validates resource permissions | • Protects prompt access<br>• Validates prompt permissions | • (Self-referential) |

## State Machine Transitions and Macro Interactions

| State | `#[mcp_server]` | `#[mcp_client]` | `#[mcp_host]` | Component Macros |
|-------|-----------------|-----------------|---------------|------------------|
| **Development** | • Validates basic configuration<br>• Checks transport settings | • Validates server references<br>• Checks transport compatibility | • Validates component references<br>• Checks orchestration settings | • Validate parent server<br>• Check implementation requirements |
| **Build** | • Registers components<br>• Generates endpoint handlers | • Generates client methods<br>• Builds transport clients | • Generates orchestration code<br>• Builds component connections | • Generate specialized handlers<br>• Build validation middleware |
| **Deployment** | • Configures for environment<br>• Sets up transport listeners | • Configures connection settings<br>• Sets up reconnection logic | • Configures component orchestration<br>• Sets up lifecycle management | • Configure specialized settings<br>• Set up caching strategies |
| **Runtime** | • Manages component lifecycle<br>• Handles requests<br>• Monitors health | • Manages server connections<br>• Auto-refreshes capabilities<br>• Handles reconnection | • Manages component lifecycle<br>• Routes requests<br>• Monitors system health | • Handle specialized requests<br>• Manage caching<br>• Monitor performance |

## Macro State Validation Matrix

This matrix shows how each macro's state affects validation in the state machine:

| Macro | Valid State | Invalid State | Warning State | Auto-Fix Actions |
|-------|-------------|---------------|---------------|------------------|
| `#[mcp_server]` | • All required attributes set<br>• Valid transport config<br>• Component handlers registered | • Missing required attributes<br>• Invalid transport config<br>• No components registered | • Incomplete documentation<br>• Non-optimal transport config<br>• Missing optional handlers | • Generate config template<br>• Suggest transport options<br>• Generate handler stubs |
| `#[mcp_client]` | • Valid server reference<br>• Compatible transport<br>• Auth configuration set | • Missing server reference<br>• Incompatible transport<br>• Missing required auth | • Server only available as mock<br>• Non-optimal transport<br>• Incomplete error handling | • Generate mock server<br>• Auto-select compatible transport<br>• Generate error handling |
| `#[mcp_host]` | • Valid component references<br>• Compatible components<br>• Complete configuration | • Missing component references<br>• Incompatible components<br>• Incomplete configuration | • Some components only as mocks<br>• Sub-optimal component config<br>• Incomplete orchestration | • Generate mock components<br>• Optimize component config<br>• Generate orchestration code |
| `#[tools]` | • Valid parent server<br>• Valid parameter types<br>• Complete implementation | • Missing parent server<br>• Invalid parameter types<br>• Incomplete implementation | • Incomplete documentation<br>• Non-optimal parameter types<br>• Missing error handling | • Link to parent server<br>• Suggest parameter type fixes<br>• Generate error handling |
| `#[resources]` | • Valid parent server<br>• Valid URI templates<br>• Complete implementation | • Missing parent server<br>• Invalid URI templates<br>• Incomplete implementation | • Incomplete documentation<br>• Non-optimal URI templates<br>• Missing caching config | • Link to parent server<br>• Fix URI templates<br>• Generate caching config |
| `#[prompts]` | • Valid parent server<br>• Valid template syntax<br>• Complete implementation | • Missing parent server<br>• Invalid template syntax<br>• Incomplete implementation | • Incomplete documentation<br>• Non-optimal template syntax<br>• Missing error handling | • Link to parent server<br>• Fix template syntax<br>• Generate error handling |
| `#[auth]` | • Valid parent server<br>• Protected components exist<br>• Complete auth handlers | • Missing parent server<br>• No protected components<br>• Incomplete auth handlers | • Incomplete documentation<br>• Minimal protection<br>• Basic auth only | • Link to parent server<br>• Suggest component protection<br>• Generate auth handler stubs |

## Transport-Specific Macro Interactions

| Transport | Server Macro Behavior | Client Macro Behavior | Component Macro Interactions |
|-----------|----------------------|----------------------|------------------------------|
| **stdio** | • Reads from stdin<br>• Writes to stdout<br>• Synchronous processing | • Writes to server stdin<br>• Reads from server stdout<br>• Process management | • Tools: Direct function calls<br>• Resources: File-based access<br>• Prompts: In-memory rendering |
| **SSE** | • HTTP server with SSE endpoint<br>• Long-polling for requests<br>• Asynchronous processing | • HTTP client with SSE connection<br>• Polling for requests<br>• Connection management | • Tools: HTTP POST requests<br>• Resources: HTTP GET requests<br>• Prompts: HTTP GET with templates |
| **WebSocket** | • WebSocket server<br>• Bidirectional communication<br>• Asynchronous processing | • WebSocket client<br>• Bidirectional communication<br>• Connection management | • Tools: WebSocket messages<br>• Resources: WebSocket messages<br>• Prompts: WebSocket messages |
| **Mixed** | • Multiple transport endpoints<br>• Transport-specific processing<br>• Unified component handling | • Multiple transport clients<br>• Transport selection logic<br>• Unified API | • Component-specific transport selection<br>• Optimal transport per component<br>• Fallback mechanisms |

## Macro Configuration Dependency Matrix

This matrix shows how configuration options in one macro affect other macros:

| Configuration Option | Affects Server | Affects Client | Affects Host | Affects Components |
|----------------------|----------------|---------------|--------------|-------------------|
| `transport` | • Determines listener type<br>• Sets message format<br>• Configures endpoints | • Must match server transport<br>• Determines connection method<br>• Sets message format | • Must coordinate transports<br>• Determines routing logic<br>• Sets orchestration strategy | • Determines access method<br>• Sets serialization format<br>• Configures caching strategy |
| `auth` | • Enables auth middleware<br>• Sets auth handlers<br>• Configures auth storage | • Sets auth credentials<br>• Manages auth tokens<br>• Handles auth errors | • Centralizes auth config<br>• Manages auth state<br>• Coordinates auth between components | • Determines access control<br>• Sets permission requirements<br>• Configures protected operations |
| `cache` | • Configures server-side caching<br>• Sets cache invalidation<br>• Manages cache storage | • Configures client-side caching<br>• Sets cache refresh<br>• Manages cache storage | • Coordinates caching strategy<br>• Manages shared caches<br>• Optimizes cache usage | • Sets component-specific caching<br>• Determines cacheable elements<br>• Configures invalidation strategy |

## Macro Lifecycle and State Machine Interaction

| Lifecycle Phase | Server Macro State | Client Macro State | Host Macro State | Component Macro States |
|-----------------|-------------------|-------------------|-----------------|------------------------|
| **Initialization** | • Loading configuration<br>• Registering components<br>• Setting up listeners | • Loading configuration<br>• Discovering servers<br>• Setting up connections | • Loading configuration<br>• Initializing components<br>• Setting up orchestration | • Registering with server<br>• Loading specialized config<br>• Setting up handlers |
| **Connection** | • Accepting connections<br>• Authenticating clients<br>• Establishing sessions | • Connecting to servers<br>• Authenticating<br>• Establishing sessions | • Coordinating connections<br>• Managing authentication<br>• Establishing component sessions | • Participating in connection setup<br>• Validating capabilities<br>• Initializing state |
| **Operation** | • Processing requests<br>• Managing resources<br>• Monitoring health | • Sending requests<br>• Receiving responses<br>• Monitoring connections | • Routing requests<br>• Orchestrating components<br>• Monitoring system | • Handling specialized requests<br>• Managing component state<br>• Performing operations |
| **Refresh** | • Updating capabilities<br>• Notifying clients<br>• Refreshing state | • Discovering changes<br>• Updating capabilities<br>• Refreshing state | • Coordinating updates<br>• Propagating changes<br>• Refreshing system state | • Updating specialized capabilities<br>• Notifying dependents<br>• Refreshing component state |
| **Shutdown** | • Notifying clients<br>• Saving state<br>• Closing connections | • Handling disconnection<br>• Saving state<br>• Cleanup | • Coordinating shutdown<br>• Saving system state<br>• Component cleanup | • Specialized cleanup<br>• Saving component state<br>• Resource release |

This comprehensive dependency matrix provides a detailed view of how the different macros in the MCPR framework relate to each other and interact with the state machine validation system throughout the development, build, deployment, and runtime phases.
