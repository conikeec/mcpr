#!/bin/bash
set -e

# Change to the project root directory
cd "$(dirname "$0")"

# Save our current directory
EXAMPLE_DIR=$(pwd)

# Build the server and client
echo "Building server and client (with run() fix)..."
# Since we're in a workspace, we need to build from the workspace root
cd ../../ && cargo build -p mcpr-example-server -p mcpr-example-client && cd - > /dev/null

# Get the path to the executables in the main workspace target directory
SERVER_PATH="/Users/chetanconikee/mcp/mcpr/target/debug/mcpr-example-server"
CLIENT_PATH="/Users/chetanconikee/mcp/mcpr/target/debug/mcpr-example-client"

# Make sure the executables exist
if [[ ! -f "$SERVER_PATH" ]]; then
  echo "Error: Server executable not found at $SERVER_PATH"
  exit 1
fi

if [[ ! -f "$CLIENT_PATH" ]]; then
  echo "Error: Client executable not found at $CLIENT_PATH"
  exit 1
fi

# Run the client, which will start the server as a subprocess
echo "Starting client (which will launch the server as a subprocess)..."
echo "Note: The server is now using run() instead of wait_for_shutdown()"
echo "      This keeps the server running and properly handling stdin/stdout"
"$CLIENT_PATH" --server-path="$SERVER_PATH" --debug

echo ""
echo "Test completed." 