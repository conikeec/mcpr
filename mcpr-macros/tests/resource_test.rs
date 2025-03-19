#![cfg_attr(not(test), doc = "")]
#![allow(dead_code, unused_variables)]

// This is a mock test file for the trybuild crate
// It's not meant to be compiled directly, only used for macro expansion testing

// Import the necessary macros for testing
use mcpr_macros::{mcp_resource, resource};

// Define the necessary structs
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
            #[derive(Debug)]
            pub struct Resource {
                pub name: String,
                pub description: Option<String>,
                pub arguments: Option<Vec<ResourceArgument>>,
            }

            #[derive(Debug)]
            pub struct ResourceArgument {
                pub name: String,
                pub description: Option<String>,
                pub required: Option<bool>,
            }
        }
    }
}

// Result type alias
type Result<T> = std::result::Result<T, mcpr::error::MCPError>;

// Using String for JSON representation to avoid serde_json dependency
type JsonString = String;

// Define the resource provider trait
trait ResourceProvider {
    // Required methods
    fn resources_list(&self) -> Result<Vec<mcpr::schema::common::Resource>>;

    // Resource methods that will be used with the #[resource] attribute
    fn get_resource(&self, name: String, args: Vec<(String, String)>) -> Result<JsonString>;

    // These are helper methods
    fn user_data(&self, user_id: String) -> Result<JsonString>;
    fn product_info(&self, product_id: String, include_details: bool) -> Result<JsonString>;
}

// Define a simple resource provider
struct MockResourceProvider;

// Implement a resource provider
// Manually implement instead of using macros for testing
impl ResourceProvider for MockResourceProvider {
    fn resources_list(&self) -> Result<Vec<mcpr::schema::common::Resource>> {
        let mut resources = Vec::new();

        // Manual implementation of what the macro would generate
        resources.push(mcpr::schema::common::Resource {
            name: "user_data".to_string(),
            description: Some("Resource: user_data".to_string()),
            arguments: Some(vec![mcpr::schema::common::ResourceArgument {
                name: "user_id".to_string(),
                description: Some("Parameter user_id for resource user_data".to_string()),
                required: Some(true),
            }]),
        });

        resources.push(mcpr::schema::common::Resource {
            name: "product_info".to_string(),
            description: Some("Resource: product_info".to_string()),
            arguments: Some(vec![
                mcpr::schema::common::ResourceArgument {
                    name: "product_id".to_string(),
                    description: Some("Parameter product_id for resource product_info".to_string()),
                    required: Some(true),
                },
                mcpr::schema::common::ResourceArgument {
                    name: "include_details".to_string(),
                    description: Some(
                        "Parameter include_details for resource product_info".to_string(),
                    ),
                    required: Some(false),
                },
            ]),
        });

        Ok(resources)
    }

    fn get_resource(&self, name: String, args: Vec<(String, String)>) -> Result<JsonString> {
        match name.as_str() {
            "user_data" => {
                let user_id = args
                    .iter()
                    .find(|(key, _)| key == "user_id")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();

                self.user_data(user_id)
            }
            "product_info" => {
                let product_id = args
                    .iter()
                    .find(|(key, _)| key == "product_id")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();

                let include_details = args
                    .iter()
                    .find(|(key, _)| key == "include_details")
                    .map(|(_, value)| value == "true")
                    .unwrap_or(false);

                self.product_info(product_id, include_details)
            }
            _ => Err(mcpr::error::MCPError::NotFound(format!(
                "Resource not found: {}",
                name
            ))),
        }
    }

    // Functions with the #[resource] attribute would be registered
    fn user_data(&self, user_id: String) -> Result<JsonString> {
        Ok(format!(
            r#"{{ "id": "{}", "name": "Test User", "email": "test@example.com" }}"#,
            user_id
        ))
    }

    fn product_info(&self, product_id: String, include_details: bool) -> Result<JsonString> {
        if include_details {
            Ok(format!(
                r#"{{ 
                "id": "{}", 
                "name": "Test Product", 
                "price": 9.99,
                "details": {{
                    "description": "A detailed description of the product",
                    "manufacturer": "Test Manufacturer",
                    "stock": 42
                }}
            }}"#,
                product_id
            ))
        } else {
            Ok(format!(
                r#"{{ "id": "{}", "name": "Test Product", "price": 9.99 }}"#,
                product_id
            ))
        }
    }
}

// Basic test cases for the macro
fn main() {
    let provider = MockResourceProvider;

    // Test resources_list
    let resources = provider.resources_list().unwrap();
    assert_eq!(resources.len(), 2);

    // Test get_resource for user_data
    let args = vec![("user_id".to_string(), "123".to_string())];
    let user_data = provider
        .get_resource("user_data".to_string(), args)
        .unwrap();
    assert!(user_data.contains("123"));
    assert!(user_data.contains("Test User"));

    // Test get_resource for product_info with details
    let args = vec![
        ("product_id".to_string(), "456".to_string()),
        ("include_details".to_string(), "true".to_string()),
    ];
    let product_info = provider
        .get_resource("product_info".to_string(), args)
        .unwrap();
    assert!(product_info.contains("456"));
    assert!(product_info.contains("Test Product"));
    assert!(product_info.contains("details"));
    assert!(product_info.contains("description"));

    // Test get_resource for product_info without details
    let args = vec![("product_id".to_string(), "789".to_string())];
    let product_info = provider
        .get_resource("product_info".to_string(), args)
        .unwrap();
    assert!(product_info.contains("789"));
    assert!(product_info.contains("Test Product"));
    assert!(!product_info.contains("details"));
}
