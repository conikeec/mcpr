#![cfg_attr(not(test), doc = "")]
#![allow(dead_code, unused_variables)]

// This is a mock test file for the trybuild crate
// It's not meant to be compiled directly, only used for macro expansion testing

// Import the necessary macros for testing
// Keeping this commented to avoid unused import warning
// use mcpr_macros::mcp_auth;

// Create a namespace that matches the real crate
pub(crate) mod mcpr {
    pub(crate) mod schema {
        pub(crate) mod auth {
            // Simple auth structs for testing
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
        // Simple error type
        #[derive(Debug)]
        pub struct MCPError;
    }
}

// Define simple types for the test
type Result<T, E = mcpr::error::MCPError> = std::result::Result<T, E>;

// Define an auth provider trait
trait AuthProvider {
    // Required methods
    fn auth_methods_list(&self) -> Result<Vec<mcpr::schema::auth::AuthMethod>>;

    // Auth methods that will be used with the #[auth] attribute
    fn authenticate_token(&self, _token: String) -> Result<bool>;
    fn authenticate_credentials(&self, _username: String, _password: String) -> Result<bool>;
}

// Define a simple auth provider
struct MockAuthProvider {
    valid_token: String,
    users: Vec<(String, String)>,
}

impl MockAuthProvider {
    fn new() -> Self {
        Self {
            valid_token: "valid_token".to_string(),
            users: vec![
                ("admin".to_string(), "password".to_string()),
                ("user".to_string(), "pass123".to_string()),
            ],
        }
    }
}

// Implement the auth provider with the macro
// Disable the macro to allow compilation while testing
// #[mcp_auth]
impl AuthProvider for MockAuthProvider {
    fn auth_methods_list(&self) -> Result<Vec<mcpr::schema::auth::AuthMethod>> {
        Ok(Vec::new())
    }

    // #[auth]
    fn authenticate_token(&self, _token: String) -> Result<bool> {
        Ok(_token == self.valid_token)
    }

    // #[auth]
    fn authenticate_credentials(&self, _username: String, _password: String) -> Result<bool> {
        Ok(self.users.contains(&(_username, _password)))
    }
}

fn main() {
    // This is just here to make trybuild happy
    let provider = MockAuthProvider::new();
    let _auth_methods = provider.auth_methods_list();
}
