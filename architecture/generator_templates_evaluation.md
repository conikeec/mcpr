# Generator Templates Evaluation

## Overview

This document evaluates the approach to generator templates in the enhanced mcpr library. It assesses whether to preserve or replace the existing generator templates and outlines a design for specification-based generation.

## Current Generator Templates

The existing mcpr library includes a project generator that creates template projects with:

1. **Client and server implementations** for different transport types (stdio, SSE)
2. **Project structure** with separate crates for client and server
3. **Test scripts** for running and testing the generated components
4. **Basic examples** of tool implementations

The current approach uses hardcoded templates with minimal customization options.

## Evaluation of Current Approach

### Strengths
- Simple implementation with direct file generation
- Provides a working starting point for users
- Includes test scripts and examples

### Limitations
- Limited customization options
- Hardcoded templates are difficult to maintain
- No support for the new macro-based approach
- Doesn't integrate with the YAML configuration system
- Limited transport options (no WebSocket support)
- No support for mixed transport configurations
- No integration with cloud deployment options

## Decision: Enhanced Generator Templates

**Decision: Preserve and enhance the generator templates approach rather than replacing it entirely.**

Rationale:
1. The existing approach provides a solid foundation that users are familiar with
2. Enhancing rather than replacing allows for backward compatibility
3. The template generation capability is valuable for quick project setup
4. The new macro-based approach can be integrated into the existing framework

## Specification-Based Generation Design

### Generator Architecture

The enhanced generator will use a specification-based approach:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│                 │     │                 │     │                 │
│  Specification  │────▶│    Generator    │────▶│  Output Project │
│                 │     │                 │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                        │
        ▼                       ▼                        ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│                 │     │                 │     │                 │
│  Template Files │     │ Template Engine │     │  Project Files  │
│                 │     │                 │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### Specification Format

The generator will use a YAML-based specification format:

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

### Template Files

Template files will be organized by component type and feature:

```
templates/
├── server/
│   ├── base/
│   │   ├── Cargo.toml.tmpl
│   │   └── src/
│   │       └── main.rs.tmpl
│   ├── prompts/
│   │   └── prompts.rs.tmpl
│   ├── resources/
│   │   └── resources.rs.tmpl
│   └── tools/
│       └── tools.rs.tmpl
├── client/
│   ├── base/
│   │   ├── Cargo.toml.tmpl
│   │   └── src/
│   │       └── main.rs.tmpl
│   └── features/
│       └── multi_server.rs.tmpl
├── host/
│   ├── Cargo.toml.tmpl
│   └── src/
│       └── main.rs.tmpl
├── config/
│   ├── server.yaml.tmpl
│   ├── client.yaml.tmpl
│   └── host.yaml.tmpl
└── deployment/
    ├── docker/
    │   └── Dockerfile.tmpl
    └── aws/
        └── template.yaml.tmpl
```

### Command-Line Interface

The enhanced generator will have an improved CLI:

```bash
mcpr generate [options]
  --spec SPEC_FILE       Path to specification YAML file
  --output DIR           Output directory
  --template-dir DIR     Custom template directory (optional)
  --force                Overwrite existing files
  --dry-run              Show what would be generated without writing files
```

### Example Usage

```bash
# Generate from specification file
mcpr generate --spec my_project_spec.yaml --output ./my-project

# Quick generation with defaults
mcpr generate --name MyProject --server --client --macros --output ./my-project

# Generate with custom templates
mcpr generate --spec my_project_spec.yaml --template-dir ./my-templates --output ./my-project
```

## Integration with Macro-Based Approach

The enhanced generator will fully support the new macro-based approach:

1. **Macro-Enabled Templates**: Templates will include the new attribute macros
2. **Configuration Generation**: Will generate YAML configuration files
3. **Mixed Approach Support**: Can generate projects using both approaches

Example generated server with macros:

```rust
// Generated server.rs
use mcpr::prelude::*;

pub struct MyServer {
    // Server state
}

impl MyServer {
    pub fn new() -> Self {
        Self { /* initialize state */ }
    }
}

#[mcp_server(
    name = "MyServer",
    config = "../config/server.yaml"
)]
impl MyServer {
    // Server implementation
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

## Backward Compatibility

The enhanced generator will maintain backward compatibility:

1. **Legacy Mode**: Support for generating projects without macros
2. **Upgrade Path**: Tools to convert existing projects to the new approach
3. **Mixed Generation**: Support for adding macro-based components to existing projects

## Implementation Plan

### Phase 1: Template Engine
1. Implement the template engine with variable substitution and conditionals
2. Create the basic template structure
3. Support the existing generation capabilities

### Phase 2: Specification Format
1. Define the YAML specification format
2. Implement specification parsing and validation
3. Connect the specification to the template engine

### Phase 3: Macro Integration
1. Update templates to include the new macros
2. Generate configuration files
3. Support mixed transport options

### Phase 4: Deployment Integration
1. Add templates for containerization
2. Implement cloud provider integration
3. Support deployment configuration

## Conclusion

The enhanced generator templates approach preserves the strengths of the existing system while adding powerful new capabilities. By adopting a specification-based approach with a flexible template engine, the generator will support the new macro-based architecture while maintaining backward compatibility.

The enhanced generator will make it even easier for users to create MCP projects, with support for all the new features of the enhanced mcpr library, including macros, mixed transports, and cloud deployment.
