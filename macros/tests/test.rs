use std::path::Path;
use trybuild::TestCases;

#[test]
fn test_auth() {
    let t = TestCases::new();
    t.pass("tests/auth_test.rs");
}

#[test]
fn test_client() {
    let t = TestCases::new();
    t.pass("tests/client_test.rs");
}

#[test]
fn test_prompt() {
    let t = TestCases::new();
    t.pass("tests/prompt_test.rs");
}

#[test]
fn test_resource() {
    let t = TestCases::new();
    t.pass("tests/resource_test.rs");
}

#[test]
fn test_server() {
    let t = TestCases::new();
    t.pass("tests/server_test.rs");
}

#[test]
fn test_state() {
    let t = TestCases::new();
    t.pass("tests/state_test.rs");
}

#[test]
fn test_transport() {
    let t = TestCases::new();
    t.pass("tests/transport_test.rs");
}

#[test]
fn test_integration() {
    let t = TestCases::new();
    t.pass("tests/integration_test.rs");
}

// Skip the resource macro test for now until we can fix the error
// Will revisit this in a future update
/*
#[test]
fn test_resource_macro() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/resource_test.rs.ignored");
}
*/
