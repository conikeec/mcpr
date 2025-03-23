use mcpr::generator;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Test that project generation works and can be configured to use a local mcpr dependency.
///
/// This test:
/// 1. Generates a new project with SSE transport
/// 2. Modifies the Cargo.toml files to use the local mcpr crate (not from crates.io)
/// 3. Verifies that the dependency was correctly updated
/// 4. Compiles the generated project to ensure it builds correctly with the local dependency
///    (Currently skipped due to a template formatting issue)
#[test]
fn test_project_generation_with_local_dependency() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path().to_str().unwrap();

    // Get absolute path to the current directory (where mcpr crate is located)
    let current_dir = std::env::current_dir()?;
    let mcpr_path = current_dir.display().to_string();

    // Generate a project with a unique name
    let project_name = "test_project";
    let transport_type = "sse"; // Using SSE transport

    // Generate the project
    generator::generate_project(project_name, temp_path, transport_type)?;

    // Paths to the Cargo.toml files
    let server_cargo_path = format!("{}/{}/server/Cargo.toml", temp_path, project_name);
    let client_cargo_path = format!("{}/{}/client/Cargo.toml", temp_path, project_name);

    // Path to the client's main.rs
    let client_main_path = format!("{}/{}/client/src/main.rs", temp_path, project_name);

    // Update both Cargo.toml files to use the local dependency
    update_toml_with_local_dependency(&server_cargo_path, &mcpr_path)?;
    update_toml_with_local_dependency(&client_cargo_path, &mcpr_path)?;

    // Verify that the Cargo.toml files were updated correctly
    let server_cargo_content = std::fs::read_to_string(&server_cargo_path)?;
    let client_cargo_content = std::fs::read_to_string(&client_cargo_path)?;

    assert!(server_cargo_content.contains(&format!("path = \"{}\"", mcpr_path)));
    assert!(!server_cargo_content.contains("# mcpr = { path = \"../..\" }"));
    assert!(!server_cargo_content.contains("mcpr = \"0.2.3\""));

    assert!(client_cargo_content.contains(&format!("path = \"{}\"", mcpr_path)));
    assert!(!client_cargo_content.contains("# mcpr = { path = \"../..\" }"));
    assert!(!client_cargo_content.contains("mcpr = \"0.2.3\""));

    // Print the client main.rs content for debugging
    println!("--- Client main.rs content ---");
    let client_main_content = std::fs::read_to_string(&client_main_path)?;
    println!("{}", client_main_content);

    // Print the server main.rs content for debugging
    let server_main_path = format!("{}/{}/server/src/main.rs", temp_path, project_name);
    println!("--- Server main.rs content ---");
    let server_main_content = std::fs::read_to_string(&server_main_path)?;
    println!("{}", server_main_content);

    // Build the server to verify it compiles with the local dependency
    println!("Building server...");
    let server_build_output = Command::new("cargo")
        .current_dir(&format!("{}/{}/server", temp_path, project_name))
        .arg("build")
        .output()?;

    assert!(
        server_build_output.status.success(),
        "Server build failed: {}",
        String::from_utf8_lossy(&server_build_output.stderr)
    );

    // Build the client to verify it compiles with the local dependency
    println!("Building client...");
    let client_build_output = Command::new("cargo")
        .current_dir(&format!("{}/{}/client", temp_path, project_name))
        .arg("build")
        .output()?;

    assert!(
        client_build_output.status.success(),
        "Client build failed: {}",
        String::from_utf8_lossy(&client_build_output.stderr)
    );

    // Verify that the binaries were created
    let server_dir = format!("{}/{}/server/target/debug", temp_path, project_name);
    let server_binary_name = format!("{}-server", project_name);
    let server_binary_path = Path::new(&server_dir).join(&server_binary_name);
    let server_binary_path_exe = Path::new(&server_dir).join(format!("{}.exe", server_binary_name));

    assert!(
        server_binary_path.exists() || server_binary_path_exe.exists(),
        "Server binary was not created"
    );

    let client_dir = format!("{}/{}/client/target/debug", temp_path, project_name);
    let client_binary_name = format!("{}-client", project_name);
    let client_binary_path = Path::new(&client_dir).join(&client_binary_name);
    let client_binary_path_exe = Path::new(&client_dir).join(format!("{}.exe", client_binary_name));

    assert!(
        client_binary_path.exists() || client_binary_path_exe.exists(),
        "Client binary was not created"
    );

    // tempdir will automatically clean up the temporary directory
    Ok(())
}

/// Updates the Cargo.toml file to use a local dependency.
///
/// This function:
/// 1. Reads the existing Cargo.toml file
/// 2. Uncomments the line for the local path dependency
/// 3. Comments out the line for the crates.io version dependency
/// 4. Writes the modified content back to the file
fn update_toml_with_local_dependency(
    cargo_toml_path: &str,
    mcpr_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the current content
    let content = std::fs::read_to_string(cargo_toml_path)?;

    // Update the content
    let updated_content = content
        // Replace the commented out path dependency with the active one
        .replace(
            "# mcpr = { path = \"../..\" }",
            &format!("mcpr = {{ path = \"{}\" }}", mcpr_path),
        )
        .replace(
            "mcpr = \"0.2.3\"",
            "# mcpr version dependency removed for testing",
        );

    // Write the updated content back to the file
    std::fs::write(cargo_toml_path, updated_content)?;

    Ok(())
}
