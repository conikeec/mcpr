use mcpr_macros::mcp_transport;

// This is a mock test file that will be processed by trybuild
// It won't actually run, but it will verify that our macro expands correctly

// Mock traits to make the test compile
trait Serialize {}
trait DeserializeOwned {}

#[derive(Debug)]
enum MCPError {
    Transport(String),
}

// Define callback types
type CloseCallback = Box<dyn Fn() + Send + Sync>;
type ErrorCallback = Box<dyn Fn(&MCPError) + Send + Sync>;

// Define the trait with the same bounds as the macro-generated implementation
trait Transport {
    fn start(&mut self) -> Result<(), MCPError>;
    fn close(&mut self) -> Result<(), MCPError>;
    fn set_on_close(&mut self, callback: Option<CloseCallback>);
    fn set_on_error(&mut self, callback: Option<ErrorCallback>);
    fn set_on_message<F>(&mut self, callback: Option<F>)
    where
        F: Fn(&str) + Send + Sync + 'static;

    // Add the same bounds that the macro will generate
    fn send<T: Serialize>(&mut self, message: &T) -> Result<(), MCPError>;
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, MCPError>;
}

// Now define the struct with the transport macro
#[mcp_transport]
struct MockTransport {
    is_connected: bool,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
    on_error: Option<Box<dyn Fn(&MCPError) + Send + Sync>>,
    on_message: Option<Box<dyn Fn(&str) + Send + Sync>>,
    // Transport-specific fields
    reader: Box<dyn std::io::Read + Send>,
    writer: Box<dyn std::io::Write + Send>,
}

impl MockTransport {
    // Override the send/receive methods with custom implementations
    fn send<T: Serialize>(&mut self, _message: &T) -> Result<(), MCPError> {
        Ok(())
    }

    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, MCPError> {
        Err(MCPError::Transport("Not implemented".to_string()))
    }
}

fn main() {
    // This is just here to make trybuild happy
}
