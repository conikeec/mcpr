//! MCP CLI tool

use clap::{Parser, Subcommand};

/// MCP CLI tool
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a server stub
    GenerateServer {
        /// Name of the server
        #[arg(short, long)]
        name: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
    },

    /// Generate a client stub
    GenerateClient {
        /// Name of the client
        #[arg(short, long)]
        name: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,
    },

    /// Generate a complete "hello mcp" project with both client and server
    GenerateProject {
        /// Name of the project
        #[arg(short, long)]
        name: String,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: String,

        /// Transport type to use (stdio, sse)
        #[arg(short, long, default_value = "stdio")]
        transport: String,
    },

    /// Run a server
    RunServer {
        /// Path to the server implementation
        #[arg(short, long)]
        path: String,
    },

    /// Connect to a server as a client
    Connect {
        /// URI of the server to connect to
        #[arg(short, long)]
        uri: String,
    },

    /// Validate an MCP message
    Validate {
        /// Path to the message file
        #[arg(short, long)]
        path: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::GenerateServer { name, output } => {
            println!("Generating server stub '{}' in '{}'", name, output);
            println!("Generator functionality temporarily disabled. The generator module has been removed.");
        }
        Commands::GenerateClient { name, output } => {
            println!("Generating client stub '{}' in '{}'", name, output);
            println!("Generator functionality temporarily disabled. The generator module has been removed.");
        }
        Commands::GenerateProject {
            name,
            output,
            transport: _,
        } => {
            println!(
                "Generating complete 'hello mcp' project '{}' in '{}'",
                name, output
            );
            println!("Generator functionality temporarily disabled. The generator module has been removed.");
        }
        Commands::RunServer { path } => {
            println!("Running server from '{}'", path);
            println!("Server runner not yet implemented");
        }
        Commands::Connect { uri } => {
            println!("Connecting to server at '{}'", uri);
            println!("Client connection not yet implemented");
        }
        Commands::Validate { path } => {
            println!("Validating message from '{}'", path);
            println!("Message validation not yet implemented");
        }
    }
}
