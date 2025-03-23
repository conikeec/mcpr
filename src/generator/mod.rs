//! Module for generating MCP server and client stubs

use std::fs;
use std::io;
use std::path::Path;

mod templates;

// Import the crate version from the environment
use crate::VERSION;

/// Error type for generator operations
#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Template error: {0}")]
    Template(String),
    #[error("Invalid name: {0}")]
    InvalidName(String),
}

/// Generate a server stub
pub fn generate_server(name: &str, output_dir: &Path) -> Result<(), GeneratorError> {
    // Validate name
    if !is_valid_name(name) {
        return Err(GeneratorError::InvalidName(format!(
            "Invalid server name: {}",
            name
        )));
    }

    // Create output directory if it doesn't exist
    let server_dir = output_dir.join(name);
    fs::create_dir_all(&server_dir)?;

    // Generate server files
    generate_server_main(&server_dir, name)?;
    generate_server_cargo_toml(&server_dir, name)?;
    generate_server_readme(&server_dir, name)?;

    println!(
        "Server stub '{}' generated successfully in '{}'",
        name,
        server_dir.display()
    );
    println!("To run the server:");
    println!("  cd {}", server_dir.display());
    println!("  cargo run");

    Ok(())
}

/// Generate a client stub
pub fn generate_client(name: &str, output_dir: &Path) -> Result<(), GeneratorError> {
    // Validate name
    if !is_valid_name(name) {
        return Err(GeneratorError::InvalidName(format!(
            "Invalid client name: {}",
            name
        )));
    }

    // Create output directory if it doesn't exist
    let client_dir = output_dir.join(name);
    fs::create_dir_all(&client_dir)?;

    // Generate client files
    generate_client_main(&client_dir, name)?;
    generate_client_cargo_toml(&client_dir, name)?;
    generate_client_readme(&client_dir, name)?;

    println!(
        "Client stub '{}' generated successfully in '{}'",
        name,
        client_dir.display()
    );
    println!("To run the client:");
    println!("  cd {}", client_dir.display());
    println!("  cargo run -- --uri <server_uri>");

    Ok(())
}

/// Generate a complete "hello mcp" project with both client and server
pub fn generate_project(
    name: &str,
    output_dir: &str,
    transport_type: &str,
) -> Result<(), GeneratorError> {
    println!(
        "Generating complete 'hello mcp' project '{}' in '{}'",
        name, output_dir
    );
    println!("Using transport type: {}", transport_type);

    // Create project directory
    let project_dir = Path::new(output_dir).join(name);
    fs::create_dir_all(&project_dir)?;

    // Create server directory
    let server_dir = project_dir.join("server");
    fs::create_dir_all(&server_dir)?;

    // Create client directory
    let client_dir = project_dir.join("client");
    fs::create_dir_all(&client_dir)?;

    // Generate server
    generate_project_server(&server_dir, name, transport_type)?;

    // Generate client
    generate_project_client(&client_dir, name, transport_type)?;

    // Generate README
    generate_project_readme(&project_dir, name, transport_type)?;

    // Generate test script
    generate_project_test_script(&project_dir, name, transport_type)?;

    println!(
        "Project '{}' with {} transport generated successfully in '{}'",
        name,
        transport_type,
        project_dir.display()
    );
    println!("To run the server:");
    println!("  cd {}/{}/server", output_dir, name);
    println!("  cargo run");
    println!("To run the client:");
    println!("  cd {}/{}/client", output_dir, name);
    println!("  cargo run -- --interactive");
    println!("To run the tests:");
    println!("  cd {}/{}", output_dir, name);
    println!("  ./test.sh");
    println!("Complete 'hello mcp' project generated successfully!");

    Ok(())
}

// Helper functions

fn is_valid_name(name: &str) -> bool {
    // Check if name is a valid Rust crate name
    if name.is_empty() {
        return false;
    }

    // Must start with a letter
    if !name.chars().next().unwrap().is_ascii_alphabetic() {
        return false;
    }

    // Can contain letters, numbers, underscores, and hyphens
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn generate_server_main(server_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let main_rs = server_dir.join("src").join("main.rs");
    fs::create_dir_all(main_rs.parent().unwrap())?;

    let content = templates::SERVER_MAIN_TEMPLATE.replace("{{name}}", name);

    fs::write(main_rs, content)?;
    Ok(())
}

fn generate_server_cargo_toml(server_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let cargo_toml = server_dir.join("Cargo.toml");

    // Default to the general server template
    let content = templates::SERVER_CARGO_TEMPLATE
        .replace("{{name}}", name)
        .replace("{{version}}", VERSION);

    fs::write(cargo_toml, content)?;
    Ok(())
}

fn generate_server_readme(server_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let readme = server_dir.join("README.md");

    let content = templates::SERVER_README_TEMPLATE.replace("{{name}}", name);

    fs::write(readme, content)?;
    Ok(())
}

fn generate_client_main(client_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let main_rs = client_dir.join("src").join("main.rs");
    fs::create_dir_all(main_rs.parent().unwrap())?;

    let content = templates::CLIENT_MAIN_TEMPLATE.replace("{{name}}", name);

    fs::write(main_rs, content)?;
    Ok(())
}

fn generate_client_cargo_toml(client_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let cargo_toml = client_dir.join("Cargo.toml");

    // Default to the general client template
    let content = templates::CLIENT_CARGO_TEMPLATE
        .replace("{{name}}", name)
        .replace("{{version}}", VERSION);

    fs::write(cargo_toml, content)?;
    Ok(())
}

fn generate_client_readme(client_dir: &Path, name: &str) -> Result<(), GeneratorError> {
    let readme = client_dir.join("README.md");

    let content = templates::CLIENT_README_TEMPLATE.replace("{{name}}", name);

    fs::write(readme, content)?;
    Ok(())
}

fn generate_project_server(
    server_dir: &Path,
    name: &str,
    transport_type: &str,
) -> Result<(), GeneratorError> {
    // Create src directory
    let src_dir = server_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Generate main.rs
    let main_rs = src_dir.join("main.rs");

    // Select the appropriate template based on transport type
    let template = match transport_type {
        "stdio" => templates::STDIO_SERVER_TEMPLATE,
        "sse" => templates::SSE_SERVER_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    // Replace template variables
    let content = template.replace("{{name}}", name);
    fs::write(main_rs, content)?;

    // Generate Cargo.toml
    let cargo_toml = server_dir.join("Cargo.toml");

    // Select the appropriate Cargo template based on transport type
    let cargo_template = match transport_type {
        "stdio" => templates::STDIO_SERVER_CARGO_TEMPLATE,
        "sse" => templates::SSE_SERVER_CARGO_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    let content = cargo_template
        .replace("{{name}}", name)
        .replace("{{version}}", VERSION);
    fs::write(cargo_toml, content)?;

    Ok(())
}

fn generate_project_client(
    client_dir: &Path,
    name: &str,
    transport_type: &str,
) -> Result<(), GeneratorError> {
    // Create src directory
    let src_dir = client_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Generate main.rs
    let main_rs = src_dir.join("main.rs");

    // Select the appropriate template based on transport type
    let template = match transport_type {
        "stdio" => templates::STDIO_CLIENT_TEMPLATE,
        "sse" => templates::SSE_CLIENT_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    // Replace template variables
    let content = template.replace("{{name}}", name);
    fs::write(main_rs, content)?;

    // Generate Cargo.toml
    let cargo_toml = client_dir.join("Cargo.toml");

    // Select the appropriate Cargo template based on transport type
    let cargo_template = match transport_type {
        "stdio" => templates::STDIO_CLIENT_CARGO_TEMPLATE,
        "sse" => templates::SSE_CLIENT_CARGO_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    let content = cargo_template
        .replace("{{name}}", name)
        .replace("{{version}}", VERSION);
    fs::write(cargo_toml, content)?;

    Ok(())
}

fn generate_project_readme(
    project_dir: &Path,
    name: &str,
    transport_type: &str,
) -> Result<(), GeneratorError> {
    let readme = project_dir.join("README.md");

    // Select the appropriate README template based on transport type
    let readme_template = match transport_type {
        "stdio" => templates::PROJECT_README_STDIO_TEMPLATE,
        "sse" => templates::PROJECT_README_SSE_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    // Replace template variables
    let content = readme_template.replace("{{name}}", name);

    fs::write(readme, content)?;
    Ok(())
}

fn generate_project_test_script(
    project_dir: &Path,
    name: &str,
    transport_type: &str,
) -> Result<(), GeneratorError> {
    // Generate test script
    let test_script = project_dir.join("test.sh");

    // Select the appropriate test script template based on transport type
    let test_script_template = match transport_type {
        "stdio" => templates::STDIO_TEST_SCRIPT_TEMPLATE,
        "sse" => templates::SSE_TEST_SCRIPT_TEMPLATE,
        _ => {
            return Err(GeneratorError::Template(format!(
                "Unsupported transport type: {}. Supported types are 'stdio' and 'sse'. WebSocket transport is planned but not yet implemented.",
                transport_type
            )))
        }
    };

    // Replace template variables
    let content = test_script_template.replace("{{name}}", name);
    fs::write(&test_script, content)?;
    make_executable(&test_script)?;

    Ok(())
}

/// Make a file executable on Unix systems
fn make_executable(file_path: &Path) -> Result<(), GeneratorError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(file_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(file_path, perms)?;
    }

    Ok(())
}
