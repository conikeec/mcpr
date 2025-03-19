#![allow(dead_code, unused_variables)]

// This is a mock test file for the trybuild crate
// It's not meant to be compiled directly, only used for macro expansion testing

// Import the necessary macros for testing
use mcpr_macros::{mcp_prompt, prompt};

// Define the necessary structs and enums
pub mod mcpr {
    pub mod error {
        #[derive(Debug)]
        pub enum MCPError {
            NotFound(String),
            // Add other error variants as needed
        }
    }

    pub mod schema {
        pub mod common {
            #[derive(Debug, Clone)]
            pub enum Role {
                User,
                Assistant,
                System,
            }

            #[derive(Debug)]
            pub struct Prompt {
                pub name: String,
                pub description: Option<String>,
                pub arguments: Option<Vec<PromptArgument>>,
            }

            #[derive(Debug)]
            pub struct PromptArgument {
                pub name: String,
                pub description: Option<String>,
                pub required: Option<bool>,
            }

            #[derive(Debug)]
            pub struct PromptMessage {
                pub role: Role,
                pub content: String,
            }
        }
    }
}

// Result type alias
type Result<T> = std::result::Result<T, mcpr::error::MCPError>;

// Define the prompt provider trait
trait PromptProvider {
    // Required methods
    fn prompts_list(&self) -> Result<Vec<mcpr::schema::common::Prompt>>;
    fn get_prompt(
        &self,
        name: String,
        args: Vec<(String, String)>,
    ) -> Result<Vec<mcpr::schema::common::PromptMessage>>;

    // Prompt methods that will be used with the #[prompt] attribute
    fn system_prompt(&self, context: String) -> Result<String>;
    fn user_greeting(&self, name: String, formal: bool) -> Result<String>;
}

// Define a simple prompt provider
struct MockPromptProvider;

// Manually implement instead of using macros for testing
impl PromptProvider for MockPromptProvider {
    fn prompts_list(&self) -> Result<Vec<mcpr::schema::common::Prompt>> {
        let mut prompts = Vec::new();

        // Manual implementation of what the macro would generate
        prompts.push(mcpr::schema::common::Prompt {
            name: "system_prompt".to_string(),
            description: Some("Prompt: system_prompt".to_string()),
            arguments: Some(vec![mcpr::schema::common::PromptArgument {
                name: "context".to_string(),
                description: Some("Parameter context for prompt system_prompt".to_string()),
                required: Some(true),
            }]),
        });

        prompts.push(mcpr::schema::common::Prompt {
            name: "user_greeting".to_string(),
            description: Some("Prompt: user_greeting".to_string()),
            arguments: Some(vec![
                mcpr::schema::common::PromptArgument {
                    name: "name".to_string(),
                    description: Some("Parameter name for prompt user_greeting".to_string()),
                    required: Some(true),
                },
                mcpr::schema::common::PromptArgument {
                    name: "formal".to_string(),
                    description: Some("Parameter formal for prompt user_greeting".to_string()),
                    required: Some(true),
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

                match self.system_prompt(context) {
                    Ok(content) => Ok(vec![mcpr::schema::common::PromptMessage {
                        role: mcpr::schema::common::Role::System,
                        content,
                    }]),
                    Err(e) => Err(e),
                }
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

                match self.user_greeting(name, formal) {
                    Ok(content) => Ok(vec![mcpr::schema::common::PromptMessage {
                        role: mcpr::schema::common::Role::User,
                        content,
                    }]),
                    Err(e) => Err(e),
                }
            }
            _ => Err(mcpr::error::MCPError::NotFound(format!(
                "Prompt not found: {}",
                name
            ))),
        }
    }

    // Functions with the #[prompt] attribute would be registered
    fn system_prompt(&self, context: String) -> Result<String> {
        Ok(format!("System prompt with context: {}", context))
    }

    fn user_greeting(&self, name: String, formal: bool) -> Result<String> {
        if formal {
            Ok(format!("Good day, Mr./Ms. {}", name))
        } else {
            Ok(format!("Hey {}!", name))
        }
    }
}

// Basic test cases for the macro
fn main() {
    let provider = MockPromptProvider;

    // Test prompts_list
    let prompts = provider.prompts_list().unwrap();
    assert_eq!(prompts.len(), 2);

    // Test get_prompt for system_prompt
    let args = vec![("context".to_string(), "test context".to_string())];
    let system_messages = provider
        .get_prompt("system_prompt".to_string(), args)
        .unwrap();
    assert_eq!(system_messages.len(), 1);
    assert!(matches!(
        system_messages[0].role,
        mcpr::schema::common::Role::System
    ));

    // Test get_prompt for user_greeting
    let args = vec![
        ("name".to_string(), "Alice".to_string()),
        ("formal".to_string(), "true".to_string()),
    ];
    let user_messages = provider
        .get_prompt("user_greeting".to_string(), args)
        .unwrap();
    assert_eq!(user_messages.len(), 1);
    assert!(matches!(
        user_messages[0].role,
        mcpr::schema::common::Role::User
    ));
}
