use mcpr::generator;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// Helper function to get the path to the mcpr crate
fn get_mcpr_path() -> Result<String, Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    Ok(current_dir.display().to_string())
}

/// Helper function to create a temp directory and generate a project
fn create_test_project(
    transport_type: &str,
) -> Result<(tempfile::TempDir, String), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path().to_str().unwrap();

    // Generate a project with a unique name
    let project_name = format!("test_project_{}", transport_type);

    println!(
        "Generating project '{}' with {} transport",
        project_name, transport_type
    );

    // Generate the project
    generator::generate_project(&project_name, temp_path, transport_type)?;

    Ok((temp_dir, project_name))
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

/// Verify that the binary was created
fn verify_binary_exists(
    base_dir: &str,
    project_name: &str,
    component_type: &str,
    transport_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let binary_dir = format!("{}/{}/target/debug", base_dir, component_type);
    let binary_name = format!("{}-{}", project_name, component_type);
    let binary_path = Path::new(&binary_dir).join(&binary_name);
    let binary_path_exe = Path::new(&binary_dir).join(format!("{}.exe", binary_name));

    assert!(
        binary_path.exists() || binary_path_exe.exists(),
        "{} binary was not created for {} transport",
        component_type,
        transport_type
    );

    Ok(())
}

/// Parameterized test function for template testing
fn test_template(
    transport_type: &str,
    component_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\n===== Testing {} {} template =====",
        transport_type, component_type
    );

    // Get absolute path to mcpr crate
    let mcpr_path = get_mcpr_path()?;

    // Create test project
    let (temp_dir, project_name) = create_test_project(transport_type)?;
    let temp_path = temp_dir.path().to_str().unwrap();

    // Update Cargo.toml with local dependency
    let cargo_path = format!(
        "{}/{}/{}/Cargo.toml",
        temp_path, project_name, component_type
    );
    update_toml_with_local_dependency(&cargo_path, &mcpr_path)?;

    // Verify the update was correct
    let cargo_content = std::fs::read_to_string(&cargo_path)?;
    assert!(cargo_content.contains(&format!("path = \"{}\"", mcpr_path)));
    assert!(!cargo_content.contains("# mcpr = { path = \"../..\" }"));
    assert!(!cargo_content.contains("mcpr = \"0.2.3\""));

    // Build the component
    println!("Building {} {}...", transport_type, component_type);
    let build_output = Command::new("cargo")
        .current_dir(&format!(
            "{}/{}/{}",
            temp_path, project_name, component_type
        ))
        .arg("build")
        .output()?;

    assert!(
        build_output.status.success(),
        "{} {} build failed: {}",
        transport_type,
        component_type,
        String::from_utf8_lossy(&build_output.stderr)
    );

    // Verify binary exists
    verify_binary_exists(
        &format!("{}/{}", temp_path, project_name),
        &project_name,
        component_type,
        transport_type,
    )?;

    println!(
        "{} {} template test completed successfully",
        transport_type, component_type
    );

    Ok(())
}

/// Test SSE server template generation and compilation
#[test]
fn test_sse_server_template() -> Result<(), Box<dyn std::error::Error>> {
    test_template("sse", "server")
}

/// Test SSE client template generation and compilation
#[test]
fn test_sse_client_template() -> Result<(), Box<dyn std::error::Error>> {
    test_template("sse", "client")
}

/// Test stdio server template generation and compilation
#[test]
fn test_stdio_server_template() -> Result<(), Box<dyn std::error::Error>> {
    test_template("stdio", "server")
}

/// Test stdio client template generation and compilation
#[test]
fn test_stdio_client_template() -> Result<(), Box<dyn std::error::Error>> {
    test_template("stdio", "client")
}
