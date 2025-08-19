# OctoFHIR MCP Server - Development Commands
# Usage: just <command>

# Default recipe (shows available commands)
default:
    just --list

# Build the project
build:
    cargo build

# Build for release
build-release:
    cargo build --release

# Run tests
test:
    cargo test

# Run specific test
test-specific test_name:
    cargo test {{test_name}}

# Check code without building (fast)
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt -- --check

# Run linter
lint:
    cargo clippy

# Run linter with pedantic warnings
lint-pedantic:
    cargo clippy -- -W clippy::pedantic

# Fix linting issues where possible
lint-fix:
    cargo clippy --fix

# Build documentation
docs:
    cargo doc --open

# Build documentation without opening
docs-build:
    cargo doc

# Clean build artifacts
clean:
    cargo clean

# Update dependencies
update:
    cargo update

# Run security audit
audit:
    cargo audit

# Install development tools
install-tools:
    cargo install cargo-audit
    cargo install cargo-outdated
    npm install -g @modelcontextprotocol/inspector

# Run the main MCP server binary
run *args:
    cargo run --bin octofhir-mcp {{args}}

# Run the MCP server with stdio transport only
run-stdio:
    cargo run --bin octofhir-mcp -- --transport stdio

# Run the MCP server with HTTP transport only
run-http PORT="3000":
    cargo run --bin octofhir-mcp -- --transport http --port {{PORT}}

# Run the MCP server with both stdio and HTTP transports
run-both PORT="3000":
    cargo run --bin octofhir-mcp -- --transport both --port {{PORT}}

# Run the benchmark binary
benchmark:
    cargo run --bin benchmark

# Run server validation tool
validate:
    cargo run --bin validate-server

# Start MCP Inspector for testing
inspect:
    npx @modelcontextprotocol/inspector

# Start MCP server with SSE for testing with MCP Inspector
serve-sse PORT="3005":
    @echo "🚀 Starting OctoFHIR MCP Server with SSE transport"
    @echo "   HTTP Server: http://localhost:{{PORT}}"
    @echo "   Health Check: curl http://localhost:{{PORT}}/health"
    @echo "   Tools List: curl http://localhost:{{PORT}}/mcp/tools/list" 
    @echo "   SSE Stream: http://localhost:{{PORT}}/sse"
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    cargo run --bin octofhir-mcp -- --transport http --port {{PORT}}

# Start MCP server with SSE and debug logging
serve-sse-debug PORT="3005":
    @echo "🔍 Starting OctoFHIR MCP Server with SSE transport (DEBUG MODE)"
    @echo "   HTTP Server: http://localhost:{{PORT}}"
    @echo "   Health Check: curl http://localhost:{{PORT}}/health"
    @echo "   Tools List: curl http://localhost:{{PORT}}/mcp/tools/list"
    @echo "   SSE Stream: http://localhost:{{PORT}}/sse" 
    @echo "   Use Ctrl+C to stop server"
    @echo ""
    RUST_LOG=debug cargo run --bin octofhir-mcp -- --transport http --port {{PORT}}

# Test SSE connection with curl
test-sse PORT="3005":
    @echo "🧪 Testing SSE connection to MCP server"
    @echo "   Connecting to: http://localhost:{{PORT}}/sse"
    @echo "   Press Ctrl+C to stop"
    @echo ""
    curl -N -H "Accept: text/event-stream" "http://localhost:{{PORT}}/sse?client_id=test-client"

# Test MCP server health and endpoints
test-server PORT="3005":
    @echo "🩺 Testing MCP server endpoints"
    @echo ""
    @echo "1. Health Check:"
    curl -s http://localhost:{{PORT}}/health | jq '.' || curl -s http://localhost:{{PORT}}/health
    @echo ""
    @echo ""
    @echo "2. Tools List:"
    curl -s http://localhost:{{PORT}}/mcp/tools/list | jq '.' || curl -s http://localhost:{{PORT}}/mcp/tools/list
    @echo ""
    @echo ""
    @echo "3. FHIRPath Evaluate Test:"
    curl -s -X POST -H "Content-Type: application/json" \
        -d '{"arguments": {"expression": "Patient.name.family", "resource": {"resourceType": "Patient", "name": [{"family": "Doe", "given": ["John"]}]}}}' \
        http://localhost:{{PORT}}/mcp/tools/fhirpath_evaluate | jq '.' || \
    curl -s -X POST -H "Content-Type: application/json" \
        -d '{"arguments": {"expression": "Patient.name.family", "resource": {"resourceType": "Patient", "name": [{"family": "Doe", "given": ["John"]}]}}}' \
        http://localhost:{{PORT}}/mcp/tools/fhirpath_evaluate
    @echo ""

# Start server and open MCP Inspector for easy testing
inspector PORT="3005":
    @echo "🔧 Starting MCP server and MCP Inspector for testing"
    @echo ""
    @echo "Step 1: Starting MCP server on port {{PORT}}..."
    @echo "Step 2: Opening MCP Inspector..."
    @echo ""
    @echo "In MCP Inspector, use the following connection:"
    @echo "  Transport: HTTP"
    @echo "  URL: http://localhost:{{PORT}}/mcp"
    @echo "  SSE URL: http://localhost:{{PORT}}/sse"
    @echo ""
    @echo "Use Ctrl+C to stop both server and inspector"
    @echo ""
    @(cargo run --bin octofhir-mcp -- --transport http --port {{PORT}} &) && \
    sleep 3 && \
    npx @modelcontextprotocol/inspector

# Complete testing workflow for SSE and MCP functionality
test-complete PORT="3005":
    @echo "🧪 Complete testing workflow for MCP server with SSE"
    @echo ""
    @echo "This will:"
    @echo "1. Start the MCP server"
    @echo "2. Test all endpoints"
    @echo "3. Test SSE connection"
    @echo "4. Show MCP Inspector connection info"
    @echo ""
    @echo "Starting server in background..."
    @(cargo run --bin octofhir-mcp -- --transport http --port {{PORT}} > /tmp/mcp-server.log 2>&1 &) && \
    sleep 3 && \
    echo "Server started, running tests..." && \
    just test-server {{PORT}} && \
    echo "" && \
    echo "🎉 All tests completed!" && \
    echo "" && \
    echo "💡 To use with MCP Inspector:" && \
    echo "   just inspector {{PORT}}" && \
    echo "" && \
    echo "💡 To test SSE manually:" && \
    echo "   just test-sse {{PORT}}" && \
    echo "" && \
    echo "💡 To stop the background server:" && \
    echo "   pkill -f octofhir-mcp" && \
    echo ""

# Run all checks (format, lint, test)
ci: fmt-check lint test

# Development workflow - format, check, test
dev: fmt check test

# Full development cycle
full: clean fmt lint test docs-build

# Watch for changes and run checks
watch:
    cargo watch -x check -x test

# Run with debug logging (stdio)
run-debug:
    RUST_LOG=debug cargo run --bin octofhir-mcp -- --transport stdio

# Run with debug logging (HTTP)
run-debug-http PORT="3000":
    RUST_LOG=debug cargo run --bin octofhir-mcp -- --transport http --port {{PORT}}

# Profile the application
profile:
    cargo build --release
    perf record --call-graph=dwarf ./target/release/octofhir-mcp --stdio-only &
    sleep 5
    pkill -INT octofhir-mcp
    perf report

# Generate flamegraph (requires cargo install flamegraph)
flamegraph:
    cargo flamegraph --bin octofhir-mcp -- --stdio-only

# Run tests with coverage (requires cargo install cargo-tarpaulin)
test-coverage:
    cargo tarpaulin --out html

# Check for outdated dependencies
outdated:
    cargo outdated

# Verify all binaries build
verify-binaries:
    cargo build --bin octofhir-mcp
    cargo build --bin benchmark  
    cargo build --bin validate-server

# Local development setup check
dev-setup-check:
    @echo "Checking development environment..."
    @echo "Cargo version: $(cargo --version)"
    @echo "Rust version: $(rustc --version)"  
    @echo "Node.js version: $(node --version || echo 'Node.js not found')"
    @echo "NPM version: $(npm --version || echo 'NPM not found')"
    @echo "MCP Inspector: $(npx @modelcontextprotocol/inspector --version 2>/dev/null || echo 'Not installed - run: npm install -g @modelcontextprotocol/inspector')"
    just verify-binaries
    @echo "✅ Development environment ready"

# Generate project metrics
metrics:
    @echo "=== Project Metrics ==="
    @echo "Lines of code:"
    find src -name "*.rs" | xargs wc -l | tail -1
    @echo "Dependencies:"
    cargo tree --depth 1 | wc -l
    @echo "Binary sizes:"
    ls -lh target/release/octofhir-mcp target/release/benchmark target/release/validate-server 2>/dev/null || echo "Build release binaries first with: just build-release"

# Quick start for new developers
quickstart:
    @echo "🚀 OctoFHIR MCP Server Quick Start"
    @echo "1. Checking development setup..."
    just dev-setup-check
    @echo "2. Running initial build..."
    just build
    @echo "3. Running tests..."
    just test
    @echo "4. Starting server in HTTP mode on port 3000..."
    @echo "   Use Ctrl+C to stop"
    @echo "   Health check: curl http://localhost:3000/health"
    @echo "   Tools list:   curl http://localhost:3000/mcp/tools/list"  
    just run-http