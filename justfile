# OctoFHIR MCP Server - Development Commands
# Usage: just <command>

# Default recipe (shows available commands)
default:
    just --list

# === BUILD COMMANDS ===

# Build the project
build:
    cargo build

# Build for release
build-release:
    cargo build --release

# Check code without building (fast)
check:
    cargo check

# Clean build artifacts
clean:
    cargo clean

# === TESTING COMMANDS ===

# Run all tests
test:
    cargo test

# Run specific test
test-specific test_name:
    cargo test {{test_name}}

# Run only unit tests
test-unit:
    cargo test --lib

# Run only integration tests
test-integration:
    cargo test --test integration_sdk

# === CODE QUALITY COMMANDS ===

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt -- --check

# Run linter
lint:
    cargo clippy

# Fix linting issues where possible
lint-fix:
    cargo clippy --fix

# Run all checks (format, lint, test)
ci: fmt-check lint test

# Development workflow - format, check, test
dev: fmt check test

# === MCP SERVER COMMANDS ===

# Run MCP server with stdio transport (recommended for MCP clients)
stdio:
    @echo "🚀 Starting OctoFHIR MCP Server with stdio transport"
    @echo "   Protocol: MCP 2025-06-18"
    @echo "   Tools: fhirpath_evaluate, fhirpath_parse, fhirpath_extract"
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    cargo run --bin octofhir-mcp stdio

# Run MCP server with HTTP transport (for web applications)
http PORT="3005":
    @echo "🚀 Starting OctoFHIR MCP Server with HTTP transport"
    @echo "   Server: http://localhost:{{PORT}}"
    @echo "   Protocol: MCP 2025-06-18" 
    @echo "   Tools: fhirpath_evaluate, fhirpath_parse, fhirpath_extract"
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    cargo run --bin octofhir-mcp http --host 127.0.0.1 --port {{PORT}}

# Show server information
info:
    cargo run --bin octofhir-mcp info

# Run server demo (shows FHIRPath evaluation example)
demo:
    cargo run --bin octofhir-mcp demo

# Validate server configuration
validate:
    cargo run --bin octofhir-validate-server

# === DEBUG COMMANDS ===

# Run with debug logging (stdio)
debug-stdio:
    @echo "🔍 Starting OctoFHIR MCP Server with stdio transport (DEBUG MODE)"
    @echo "   Debug logging enabled"
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    RUST_LOG=debug cargo run --bin octofhir-mcp stdio --log-level debug

# Run with debug logging (HTTP)
debug-http PORT="3005":
    @echo "🔍 Starting OctoFHIR MCP Server with HTTP transport (DEBUG MODE)"
    @echo "   Server: http://localhost:{{PORT}}"
    @echo "   Debug logging enabled"
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    RUST_LOG=debug cargo run --bin octofhir-mcp http --host 127.0.0.1 --port {{PORT}} --log-level debug

# === DEVELOPMENT TOOLS ===

# Install development tools
install-tools:
    cargo install cargo-audit
    cargo install cargo-outdated
    cargo install cargo-watch
    npm install -g @modelcontextprotocol/inspector

# Run security audit
audit:
    cargo audit

# Check for outdated dependencies
outdated:
    cargo outdated

# Update dependencies
update:
    cargo update

# Watch for changes and run checks
watch:
    cargo watch -x check -x test

# === MCP INSPECTOR INTEGRATION ===

# Start MCP Inspector for testing
inspect:
    @echo "🔧 Starting MCP Inspector for testing"
    @echo "   Use this to test MCP protocol compliance"
    @echo ""
    npx @modelcontextprotocol/inspector

# Start server and open MCP Inspector for easy testing
inspector PORT="3005":
    @echo "🔧 Starting MCP server and MCP Inspector for testing"
    @echo ""
    @echo "Server will start on port {{PORT}}"
    @echo "Inspector will open automatically"
    @echo ""
    @echo "In MCP Inspector, configure connection:"
    @echo "  Transport: HTTP Streamable"
    @echo "  URL: http://localhost:{{PORT}}"
    @echo ""
    @echo "Use Ctrl+C to stop both server and inspector"
    @echo ""
    @(just http {{PORT}} &) && \
    sleep 3 && \
    npx @modelcontextprotocol/inspector

# === TESTING WORKFLOWS ===

# Test FHIRPath evaluation functionality
test-fhirpath:
    @echo "🧪 Testing FHIRPath evaluation functionality"
    @echo ""
    cargo run --bin octofhir-mcp demo

# Complete testing workflow
test-complete:
    @echo "🧪 Complete testing workflow"
    @echo ""
    @echo "1. Running unit tests..."
    just test-unit
    @echo ""
    @echo "2. Running integration tests..."  
    just test-integration
    @echo ""
    @echo "3. Testing FHIRPath functionality..."
    just test-fhirpath
    @echo ""
    @echo "4. Validating server configuration..."
    just validate
    @echo ""
    @echo "🎉 All tests completed successfully!"

# === DOCUMENTATION ===

# Build and open documentation
docs:
    cargo doc --open

# Build documentation without opening
docs-build:
    cargo doc

# === BENCHMARKING ===

# Run benchmark binary
benchmark:
    cargo run --bin octofhir-benchmark --release

# === UTILITIES ===

# Development setup check
dev-setup-check:
    @echo "🔍 Checking development environment..."
    @echo "Cargo version: $(cargo --version)"
    @echo "Rust version: $(rustc --version)"
    @echo "Node.js version: $(node --version || echo 'Node.js not found')"
    @echo "NPM version: $(npm --version || echo 'NPM not found')"
    @echo ""
    @echo "Building binaries..."
    cargo build --bin octofhir-mcp
    cargo build --bin octofhir-benchmark  
    cargo build --bin octofhir-validate-server
    @echo ""
    @echo "✅ Development environment ready"

# Generate project metrics
metrics:
    @echo "=== OctoFHIR MCP Server Metrics ==="
    @echo "Lines of Rust code:"
    find src -name "*.rs" | xargs wc -l | tail -1
    @echo "Test files:"
    find tests -name "*.rs" | wc -l
    @echo "Dependencies:"
    cargo tree --depth 1 | wc -l
    @echo ""
    @echo "Binary sizes (release):"
    ls -lh target/release/octofhir-mcp 2>/dev/null || echo "Build release first: just build-release"

# Quick start for new developers
quickstart:
    @echo "🚀 OctoFHIR MCP Server Quick Start"
    @echo ""
    @echo "1. Checking development setup..."
    just dev-setup-check
    @echo ""
    @echo "2. Running tests..."
    just test
    @echo ""
    @echo "3. Server is ready! Try these commands:"
    @echo "   just stdio     # Start with stdio transport (for MCP clients)"
    @echo "   just http      # Start with HTTP transport (for web apps)"
    @echo "   just demo      # See FHIRPath evaluation demo"
    @echo "   just inspector # Test with MCP Inspector"
    @echo ""
    @echo "🎉 Ready to develop with OctoFHIR MCP Server!"

# === ALIASES FOR CONVENIENCE ===

# Alias for stdio transport
s: stdio

# Alias for HTTP transport  
h PORT="3005": (http PORT)

# Alias for demo
d: demo

# Alias for info
i: info

# Alias for build and test
bt: build test