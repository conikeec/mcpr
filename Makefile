.PHONY: check test clippy all

all: check test

# Run cargo check
check:
	cargo check

# Run all tests
test:
	cargo test

# Run clippy
clippy:
	cargo clippy -- -D warnings

# Clean the project
clean:
	cargo clean

# Build in release mode
release:
	cargo build --release 