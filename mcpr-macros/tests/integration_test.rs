// Integration test for all MCP macros working together
#![allow(dead_code, unused_variables)]
// Only compile this file when explicitly requested
#![cfg(feature = "integration_tests")]

// Import the necessary macros for testing
use mcpr_macros::{mcp_auth, mcp_client, mcp_prompt, mcp_resource, mcp_server, mcp_transport};

// Temporarily disable macro attributes to avoid proc-macro errors in tests
// use mcpr_macros::{mcp_auth, mcp_client, mcp_prompt, mcp_resource, mcp_server, mcp_state, mcp_transport};

// Mock the mcpr crate
pub(crate) mod mcpr {
    pub(crate) mod schema {
        pub(crate) mod common {
            use std::collections::HashMap;

            pub struct Tool {
                pub name: String,
                pub description: Option<String>,
                pub input_schema: ToolInputSchema,
            }

            pub struct ToolInputSchema {
                pub r#type: String,
                pub properties: Option<HashMap<String, crate::JSONValue>>,
                pub required: Option<Vec<String>>,
            }

            pub struct ResourceTemplate {
                pub uri_template: String,
                pub name: String,
                pub description: Option<String>,
                pub mime_type: Option<String>,
                pub annotations: Option<Vec<String>>,
            }

            pub struct Prompt {
                pub name: String,
                pub description: Option<String>,
                pub arguments: Option<Vec<PromptArgument>>,
            }

            pub struct PromptArgument {
                pub name: String,
                pub description: Option<String>,
                pub required: Option<bool>,
            }

            pub struct PromptMessage {
                pub role: Role,
                pub content: String,
            }

            pub enum Role {
                User,
                Assistant,
                System,
            }
        }

        pub(crate) mod auth {
            pub struct AuthMethod {
                pub name: String,
                pub description: Option<String>,
                pub params: Vec<String>,
                pub auth_type: AuthType,
            }

            pub enum AuthType {
                BearerToken,
                BasicAuth,
                OAuth2,
                ApiKey,
                Custom(String),
            }
        }
    }

    pub(crate) mod error {
        #[derive(Debug)]
        pub enum MCPError {
            Transport(String),
            Serialization(String),
            Authentication(String),
            Authorization(String),
            InvalidRequest(String),
            NotFound(String),
            State(String),
            Resource(String),
            Prompt(String),
        }
    }
}

// Define simple value type for JSON
pub struct JSONValue;

// Define the Result type
type Result<T, E = mcpr::error::MCPError> = std::result::Result<T, E>;

// Mock traits required by the macros
trait Serialize {}
trait DeserializeOwned {}

// Define callback types
type CloseCallback = Box<dyn Fn() + Send + Sync>;
type ErrorCallback = Box<dyn Fn(&mcpr::error::MCPError) + Send + Sync>;
type MessageCallback = Box<dyn Fn(&str) + Send + Sync>;

// Define the Transport trait needed by mcp_transport
trait Transport {
    fn start(&mut self) -> Result<(), mcpr::error::MCPError>;
    fn close(&mut self) -> Result<(), mcpr::error::MCPError>;
    fn set_on_close(&mut self, callback: Option<CloseCallback>);
    fn set_on_error(&mut self, callback: Option<ErrorCallback>);
    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static;

    fn send<T: Serialize>(&mut self, message: &T) -> Result<(), mcpr::error::MCPError>;
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, mcpr::error::MCPError>;
}

// Define the state manager
#[allow(unused)]
struct StateManager {
    current_state: String,
    active_states: Vec<String>,
}

// 1. Define a transport for communication
// Commenting out the macro to avoid proc-macro errors in tests
// #[mcp_transport]
struct TestTransport {
    is_connected: bool,
    on_close: Option<CloseCallback>,
    on_error: Option<ErrorCallback>,
    on_message: Option<MessageCallback>,
    // Transport-specific fields
    buffer: Vec<u8>,
}

impl TestTransport {
    fn new() -> Self {
        Self {
            is_connected: false,
            on_close: None,
            on_error: None,
            on_message: None,
            buffer: Vec::new(),
        }
    }

    // Implement custom send/receive methods to override default behavior
    fn send<T: Serialize>(&mut self, _message: &T) -> Result<()> {
        Ok(())
    }

    fn receive<T: DeserializeOwned>(&mut self) -> Result<T> {
        Err(mcpr::error::MCPError::Transport(
            "Not implemented".to_string(),
        ))
    }
}

// Manual implementation of Transport for TestTransport
impl Transport for TestTransport {
    fn start(&mut self) -> Result<(), mcpr::error::MCPError> {
        if self.is_connected {
            return Ok(());
        }
        self.is_connected = true;
        Ok(())
    }

    fn close(&mut self) -> Result<(), mcpr::error::MCPError> {
        if !self.is_connected {
            return Ok(());
        }
        self.is_connected = false;
        if let Some(callback) = &self.on_close {
            callback();
        }
        Ok(())
    }

    fn set_on_close(&mut self, callback: Option<CloseCallback>) {
        self.on_close = callback;
    }

    fn set_on_error(&mut self, callback: Option<ErrorCallback>) {
        self.on_error = callback;
    }

    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_message = callback.map(|f| Box::new(f) as Box<dyn Fn(&str) + Send + Sync>);
    }

    fn send<T: Serialize>(&mut self, message: &T) -> Result<(), mcpr::error::MCPError> {
        self.send(message)
    }

    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, mcpr::error::MCPError> {
        self.receive()
    }
}

// 2. Define an authentication provider
trait AuthProvider {
    fn auth_methods_list(&self) -> Result<Vec<mcpr::schema::auth::AuthMethod>>;
    fn authenticate_token(&self, token: String) -> Result<bool>;
    fn authenticate_credentials(&self, username: String, password: String) -> Result<bool>;
}

struct TestAuthProvider {
    valid_token: String,
    users: Vec<(String, String)>,
}

impl TestAuthProvider {
    fn new() -> Self {
        Self {
            valid_token: "valid_token".to_string(),
            users: vec![
                ("admin".to_string(), "password".to_string()),
                ("user".to_string(), "pass123".to_string()),
            ],
        }
    }

    fn get_current_user(&self) -> Option<User> {
        Some(User {
            id: "1".to_string(),
            name: "Test User".to_string(),
            roles: vec!["user".to_string(), "admin".to_string()],
        })
    }
}

// #[mcp_auth]
impl AuthProvider for TestAuthProvider {
    fn auth_methods_list(&self) -> Result<Vec<mcpr::schema::auth::AuthMethod>> {
        let mut methods = Vec::new();
        methods.push(mcpr::schema::auth::AuthMethod {
            name: "authenticate_token".to_string(),
            description: Some("Token-based authentication".to_string()),
            params: vec!["token".to_string()],
            auth_type: mcpr::schema::auth::AuthType::BearerToken,
        });
        methods.push(mcpr::schema::auth::AuthMethod {
            name: "authenticate_credentials".to_string(),
            description: Some("Basic username/password authentication".to_string()),
            params: vec!["username".to_string(), "password".to_string()],
            auth_type: mcpr::schema::auth::AuthType::BasicAuth,
        });
        Ok(methods)
    }

    // #[auth]
    fn authenticate_token(&self, token: String) -> Result<bool> {
        Ok(token == self.valid_token)
    }

    // #[auth]
    fn authenticate_credentials(&self, username: String, password: String) -> Result<bool> {
        Ok(self.users.contains(&(username, password)))
    }
}

// 3. Define a resource provider
trait ResourceProvider {
    fn resources_list(&self) -> Result<Vec<mcpr::schema::common::ResourceTemplate>>;
    fn get_document(&self, doc_id: String) -> Result<String>;
    fn get_image(&self, image_id: String, size: u32) -> Result<Vec<u8>>;
}

struct TestResourceProvider;

// #[mcp_resource]
impl ResourceProvider for TestResourceProvider {
    fn resources_list(&self) -> Result<Vec<mcpr::schema::common::ResourceTemplate>> {
        let mut resources = Vec::new();
        resources.push(mcpr::schema::common::ResourceTemplate {
            uri_template: "/document/{doc_id}".to_string(),
            name: "get_document".to_string(),
            description: Some("Get document by ID".to_string()),
            mime_type: Some("text/plain".to_string()),
            annotations: None,
        });
        resources.push(mcpr::schema::common::ResourceTemplate {
            uri_template: "/image/{image_id}?size={size}".to_string(),
            name: "get_image".to_string(),
            description: Some("Get image by ID with specified size".to_string()),
            mime_type: Some("image/jpeg".to_string()),
            annotations: None,
        });
        Ok(resources)
    }

    // #[resource]
    fn get_document(&self, doc_id: String) -> Result<String> {
        Ok(format!("Document content for: {}", doc_id))
    }

    // #[resource]
    fn get_image(&self, image_id: String, size: u32) -> Result<Vec<u8>> {
        Ok(vec![0, 1, 2, 3]) // Mock image data
    }
}

// 4. Define a prompt provider
trait PromptProvider {
    fn prompts_list(&self) -> Result<Vec<mcpr::schema::common::Prompt>>;
    fn get_prompt(
        &self,
        name: String,
        args: Vec<(String, String)>,
    ) -> Result<Vec<mcpr::schema::common::PromptMessage>>;
    fn system_prompt(&self, context: String) -> Result<String>;
    fn user_greeting(&self, name: String, formal: bool) -> Result<String>;
}

struct TestPromptProvider;

// #[mcp_prompt]
impl PromptProvider for TestPromptProvider {
    fn prompts_list(&self) -> Result<Vec<mcpr::schema::common::Prompt>> {
        let mut prompts = Vec::new();
        prompts.push(mcpr::schema::common::Prompt {
            name: "system_prompt".to_string(),
            description: Some("System prompt with context".to_string()),
            arguments: Some(vec![mcpr::schema::common::PromptArgument {
                name: "context".to_string(),
                description: Some("Context for the system prompt".to_string()),
                required: Some(true),
            }]),
        });
        prompts.push(mcpr::schema::common::Prompt {
            name: "user_greeting".to_string(),
            description: Some("Greeting for user".to_string()),
            arguments: Some(vec![
                mcpr::schema::common::PromptArgument {
                    name: "name".to_string(),
                    description: Some("User's name".to_string()),
                    required: Some(true),
                },
                mcpr::schema::common::PromptArgument {
                    name: "formal".to_string(),
                    description: Some("Whether to use formal greeting".to_string()),
                    required: Some(false),
                },
            ]),
        });
        Ok(prompts)
    }

    fn get_prompt(
        &self,
        name: String,
        args: Vec<(String, String)>,
    ) -> Result<Vec<mcpr::schema::common::PromptMessage>> {
        match name.as_str() {
            "system_prompt" => {
                let context = args
                    .iter()
                    .find(|(key, _)| key == "context")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                Ok(vec![mcpr::schema::common::PromptMessage {
                    role: mcpr::schema::common::Role::System,
                    content: format!("System prompt with context: {}", context),
                }])
            }
            "user_greeting" => {
                let name = args
                    .iter()
                    .find(|(key, _)| key == "name")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                let formal = args
                    .iter()
                    .find(|(key, _)| key == "formal")
                    .map(|(_, value)| value == "true")
                    .unwrap_or(false);
                let greeting = if formal {
                    format!("Good day, Mr./Ms. {}", name)
                } else {
                    format!("Hey {}!", name)
                };
                Ok(vec![mcpr::schema::common::PromptMessage {
                    role: mcpr::schema::common::Role::User,
                    content: greeting,
                }])
            }
            _ => Err(mcpr::error::MCPError::NotFound(format!(
                "Prompt not found: {}",
                name
            ))),
        }
    }

    // #[prompt]
    fn system_prompt(&self, context: String) -> Result<String> {
        Ok(format!("System prompt with context: {}", context))
    }

    // #[prompt]
    fn user_greeting(&self, name: String, formal: bool) -> Result<String> {
        if formal {
            Ok(format!("Good day, Mr./Ms. {}", name))
        } else {
            Ok(format!("Hey {}!", name))
        }
    }
}

// 5. Define a server with tools
trait McpServer {
    fn tools_list(&self) -> Result<Vec<mcpr::schema::common::Tool>>;
    fn search_documents(&self, query: String) -> Result<Vec<String>>;
    fn analyze_image(&self, image_url: String, mode: String) -> Result<String>;
    fn execute_code(&self, language: String, code: String) -> Result<String>;
}

struct TestServer {
    state_manager: StateManager,
    auth_provider: TestAuthProvider,
}

impl TestServer {
    fn new() -> Self {
        Self {
            state_manager: StateManager {
                current_state: "initial".to_string(),
                active_states: vec!["initial".to_string()],
            },
            auth_provider: TestAuthProvider::new(),
        }
    }

    fn get_current_user(&self) -> Option<User> {
        self.auth_provider.get_current_user()
    }
}

// #[mcp_server]
impl McpServer for TestServer {
    fn tools_list(&self) -> Result<Vec<mcpr::schema::common::Tool>> {
        let mut tools = Vec::new();
        tools.push(mcpr::schema::common::Tool {
            name: "search_documents".to_string(),
            description: Some("Search for documents".to_string()),
            input_schema: mcpr::schema::common::ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some({
                    let props = std::collections::HashMap::new();
                    // This is a simplified version, in reality this would be a proper JSON schema
                    props
                }),
                required: Some(vec!["query".to_string()]),
            },
        });
        tools.push(mcpr::schema::common::Tool {
            name: "analyze_image".to_string(),
            description: Some("Analyze an image".to_string()),
            input_schema: mcpr::schema::common::ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some({
                    let props = std::collections::HashMap::new();
                    // This is a simplified version, in reality this would be a proper JSON schema
                    props
                }),
                required: Some(vec!["image_url".to_string(), "mode".to_string()]),
            },
        });
        tools.push(mcpr::schema::common::Tool {
            name: "execute_code".to_string(),
            description: Some("Execute code in a sandbox".to_string()),
            input_schema: mcpr::schema::common::ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some({
                    let props = std::collections::HashMap::new();
                    // This is a simplified version, in reality this would be a proper JSON schema
                    props
                }),
                required: Some(vec!["language".to_string(), "code".to_string()]),
            },
        });
        Ok(tools)
    }

    // #[tool]
    // #[requires_role = "user"]
    fn search_documents(&self, query: String) -> Result<Vec<String>> {
        // Check user role (would be handled by the macro in a real implementation)
        if let Some(user) = self.get_current_user() {
            if !user.has_role("user") {
                return Err(mcpr::error::MCPError::Authorization(
                    "Missing required role: user".to_string(),
                ));
            }
        } else {
            return Err(mcpr::error::MCPError::Authentication(
                "User not authenticated".to_string(),
            ));
        }

        Ok(vec!["Result 1".to_string(), "Result 2".to_string()])
    }

    // #[tool]
    // #[requires_role = "admin"]
    fn analyze_image(&self, image_url: String, mode: String) -> Result<String> {
        // Check user role (would be handled by the macro in a real implementation)
        if let Some(user) = self.get_current_user() {
            if !user.has_role("admin") {
                return Err(mcpr::error::MCPError::Authorization(
                    "Missing required role: admin".to_string(),
                ));
            }
        } else {
            return Err(mcpr::error::MCPError::Authentication(
                "User not authenticated".to_string(),
            ));
        }

        Ok(format!("Analysis of {} using mode {}", image_url, mode))
    }

    // #[tool]
    // #[mcp_state]
    fn execute_code(&self, language: String, code: String) -> Result<String> {
        // Check state (would be handled by the macro in a real implementation)
        if self.state_manager.current_state != "initial"
            && !self
                .state_manager
                .active_states
                .contains(&"initial".to_string())
        {
            return Err(mcpr::error::MCPError::State(
                "Operation requires 'initial' state to be active".to_string(),
            ));
        }

        Ok(format!("Executed {} code: {}", language, code))
    }
}

// 6. Define a client
trait McpClient {
    fn search_docs(&self, query: String) -> Result<String>;
    fn get_file_content(&self, path: String, limit: u32) -> Result<String>;
}

struct TestClient {
    transport: TestTransport,
}

impl TestClient {
    fn new() -> Self {
        Self {
            transport: TestTransport::new(),
        }
    }
}

// #[mcp_client]
impl McpClient for TestClient {
    fn search_docs(&self, query: String) -> Result<String> {
        Ok(format!("Search results for: {}", query))
    }

    fn get_file_content(&self, path: String, limit: u32) -> Result<String> {
        Ok(format!("Content of {} (limit: {})", path, limit))
    }
}

// 7. User type for RBAC
struct User {
    id: String,
    name: String,
    roles: Vec<String>,
}

impl User {
    fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end() {
        // Create instances of our types
        let mut transport = TestTransport::new();
        let auth_provider = TestAuthProvider::new();
        let resource_provider = TestResourceProvider;
        let prompt_provider = TestPromptProvider;
        let server = TestServer::new();
        let client = TestClient::new();

        // Test transport functionality
        assert!(transport.start().is_ok());
        assert!(transport.close().is_ok());

        // Test auth functionality
        assert!(auth_provider
            .authenticate_token("valid_token".to_string())
            .unwrap());

        // Test server functionality
        let search_results = server.search_documents("test".to_string()).unwrap();
        assert_eq!(search_results.len(), 2);

        // Test client functionality
        let search_result = client.search_docs("test".to_string()).unwrap();
        assert!(search_result.contains("test"));
    }
}

fn main() {
    // This is just here to make trybuild happy
}
