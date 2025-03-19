# Development Scenarios and Permutations for MCPR Framework

This document provides a comprehensive table of permutations and combinations for different development scenarios in the MCPR framework, serving as a blueprint for usage and development.

## Component Dependency Matrix

| Component | Dependencies | Validation Rules | Auto Actions |
|-----------|--------------|------------------|--------------|
| `#[mcp_server]` | None | - Valid configuration file<br>- Valid transport settings | - Generate config template if missing |
| `#[mcp_client]` | Server definition | - Server exists or is mocked<br>- Compatible transport | - Auto-discover server if in same project<br>- Generate mock server if needed |
| `#[mcp_host]` | Client and Server definitions | - All referenced clients exist<br>- All referenced servers exist | - Auto-discover clients and servers<br>- Generate missing components |
| `#[resources]` | Server definition | - Parent server exists<br>- Valid URI templates | - Auto-register with parent server |
| `#[tools]` | Server definition | - Parent server exists<br>- Valid parameter types | - Auto-register with parent server<br>- Auto-refresh clients when tools change |
| `#[prompts]` | Server definition | - Parent server exists<br>- Valid template syntax | - Auto-register with parent server |
| `#[auth]` | Server definition | - Parent server exists<br>- Auth handlers for protected resources/tools | - Generate auth stubs for protected components |

## Development Scenario Permutations

| Scenario | Components | Validation Requirements | Auto Actions | Caching Options |
|----------|------------|-------------------------|--------------|-----------------|
| **Server-Only Development** | `#[mcp_server]`<br>`#[resources]`<br>`#[tools]`<br>`#[prompts]` | - Valid component definitions<br>- Consistent transport settings | - Generate test client<br>- Generate documentation | - No caching needed |
| **Client-Only Development** | `#[mcp_client]` | - Server definition exists or is mocked<br>- Compatible transport | - Generate mock server<br>- Auto-discover real server | - Cache server capabilities<br>- Cache resource metadata |
| **Host Development** | `#[mcp_host]`<br>`#[mcp_client]`<br>`#[mcp_server]` | - All referenced components exist<br>- Compatible transports | - Auto-discover components<br>- Generate missing components | - Cache client connections<br>- Cache server capabilities |
| **Full-Stack Development** | All macros | - Consistent definitions across components<br>- Compatible transports | - Auto-sync component changes<br>- Auto-refresh on changes | - Configurable caching for all components |
| **Server with Resources** | `#[mcp_server]`<br>`#[resources]` | - Valid URI templates<br>- Resource handlers implemented | - Generate resource documentation<br>- Generate test client | - Resource content caching<br>- Cache TTL configuration |
| **Server with Tools** | `#[mcp_server]`<br>`#[tools]` | - Valid parameter types<br>- Tool handlers implemented | - Generate tool documentation<br>- Generate test client | - Tool result caching<br>- Cache invalidation on updates |
| **Server with Prompts** | `#[mcp_server]`<br>`#[prompts]` | - Valid template syntax<br>- Prompt handlers implemented | - Generate prompt documentation<br>- Generate test client | - Rendered prompt caching<br>- Template compilation caching |
| **Authenticated Server** | `#[mcp_server]`<br>`#[auth]`<br>Any component | - Auth handlers for protected components<br>- Valid auth configuration | - Generate auth documentation<br>- Generate secure test client | - Auth token caching<br>- Permission caching |
| **Multi-Transport Server** | `#[mcp_server]` with multiple transports | - Valid transport configurations<br>- No transport conflicts | - Generate transport-specific clients<br>- Auto-select optimal transport | - Transport-specific caching strategies |
| **Client with Multiple Servers** | `#[mcp_client]` with multiple servers | - All servers exist or are mocked<br>- Compatible capabilities | - Auto-discover servers<br>- Generate missing mock servers | - Server-specific capability caching<br>- Connection pooling |

## Transport Combinations

| Server Transport | Client Transport | Compatibility | Validation Rules | Auto Actions |
|------------------|------------------|---------------|------------------|--------------|
| stdio | stdio | ✅ Compatible | - Path to server executable | - Auto-start server process |
| stdio | SSE | ❌ Incompatible | - Error during validation | - Suggest compatible transport |
| stdio | WebSocket | ❌ Incompatible | - Error during validation | - Suggest compatible transport |
| SSE | SSE | ✅ Compatible | - Valid server URL<br>- Network connectivity | - Auto-reconnect on failure |
| SSE | WebSocket | ❌ Incompatible | - Error during validation | - Suggest compatible transport |
| WebSocket | WebSocket | ✅ Compatible | - Valid server URL<br>- Network connectivity | - Auto-reconnect on failure |
| WebSocket | SSE | ❌ Incompatible | - Error during validation | - Suggest compatible transport |
| Mixed (Server) | Auto-detect | ✅ Compatible | - Client supports auto-detection | - Select optimal transport |

## Component Caching Options

| Component | Cacheable Elements | Cache Configuration | Invalidation Strategy |
|-----------|-------------------|---------------------|------------------------|
| `#[resources]` | - Resource content<br>- Resource metadata | ```yaml<br>resources:<br>  cache: true<br>  ttl: 3600<br>  max_size: "100MB"<br>``` | - TTL expiration<br>- Explicit invalidation<br>- Content change detection |
| `#[tools]` | - Tool results<br>- Tool schemas | ```yaml<br>tools:<br>  cache_results: true<br>  ttl: 300<br>  stateless_only: true<br>``` | - TTL expiration<br>- Parameter-based invalidation<br>- Tool definition changes |
| `#[prompts]` | - Rendered prompts<br>- Template compilation | ```yaml<br>prompts:<br>  cache_compiled: true<br>  cache_rendered: false<br>``` | - Template changes<br>- Parameter changes |
| `#[auth]` | - Authentication tokens<br>- Authorization decisions | ```yaml<br>auth:<br>  token_ttl: 3600<br>  permission_ttl: 300<br>``` | - TTL expiration<br>- User action changes<br>- Permission changes |

## Development Workflow States

| State | Components Involved | Validation Actions | Auto Actions |
|-------|---------------------|-------------------|--------------|
| **Initial Development** | Any component | - Basic syntax validation<br>- Configuration validation | - Generate config templates<br>- Generate documentation stubs |
| **Component Addition** | New component added to existing | - Dependency validation<br>- Compatibility check | - Auto-register with parent<br>- Update dependent components |
| **Component Modification** | Existing component modified | - Interface compatibility<br>- Breaking change detection | - Auto-update dependent components<br>- Generate migration code |
| **Pre-Build Validation** | All components | - Complete dependency check<br>- Configuration completeness | - Generate missing implementations<br>- Fix configuration issues |
| **Build Process** | All components | - Macro expansion validation<br>- Type checking | - Generate optimized code<br>- Apply build-specific configurations |
| **Deployment Preparation** | All components | - Environment validation<br>- Security checks | - Generate deployment configs<br>- Apply environment-specific settings |
| **Runtime Initialization** | All components | - Connection validation<br>- Capability discovery | - Establish connections<br>- Initialize caches |
| **Runtime Operation** | All components | - Health monitoring<br>- Performance tracking | - Auto-reconnect on failure<br>- Cache management |

## Mixed Transport Scenarios

| Server Components | Transport Configuration | Validation Rules | Auto Actions |
|-------------------|------------------------|------------------|--------------|
| `#[resources]` with SSE<br>`#[tools]` with WebSocket | ```yaml<br>transport:<br>  resources: "sse"<br>  tools: "websocket"<br>``` | - No port conflicts<br>- Valid configurations for each | - Configure optimal transport for each<br>- Generate client with mixed transport support |
| `#[prompts]` with stdio<br>`#[resources]` with SSE | ```yaml<br>transport:<br>  prompts: "stdio"<br>  resources: "sse"<br>``` | - Valid configurations for each<br>- No transport conflicts | - Generate specialized handlers for each<br>- Configure client with mixed transport |
| All components with auto-select | ```yaml<br>transport:<br>  auto_select: true<br>  preference: ["websocket", "sse", "stdio"]<br>``` | - All transports properly configured<br>- Fallback options available | - Select optimal transport at runtime<br>- Fallback on transport failure |

## Macro Combination Validation Rules

| Macro Combination | Valid | Validation Rules | Error Messages |
|-------------------|-------|------------------|----------------|
| `#[mcp_server]` + `#[resources]` on same impl | ❌ Invalid | - Different impl blocks required | "Server and resources must be defined in separate impl blocks" |
| `#[mcp_server]` + `#[mcp_client]` on same struct | ❌ Invalid | - Different structs required | "Server and client must be defined on different structs" |
| `#[resources]` + `#[tools]` on same impl | ❌ Invalid | - Different impl blocks required | "Resources and tools must be defined in separate impl blocks" |
| `#[mcp_host]` without clients or servers | ❌ Invalid | - At least one client or server required | "Host must reference at least one client or server" |
| `#[auth]` without protected components | ⚠️ Warning | - Auth should protect something | "Auth defined but no protected components found" |
| Multiple `#[resources]` impls for same struct | ✅ Valid | - Unique resource URIs | "Duplicate resource URI detected" |
| Multiple `#[tools]` impls for same struct | ✅ Valid | - Unique tool names | "Duplicate tool name detected" |

This comprehensive table of permutations and combinations serves as a blueprint for development with the MCPR framework, covering component dependencies, validation rules, auto actions, and caching options for various development scenarios.
