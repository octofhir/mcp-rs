# OctoFHIR MCP Server - Development Guidelines

## Overview

This project is a Model Context Protocol (MCP) server for the OctoFHIR ecosystem, providing high-performance FHIRPath evaluation and FHIR tooling through standardized MCP interfaces. It's built in Rust with a sophisticated async architecture supporting multiple transport protocols.

## Build/Configuration Instructions

### Prerequisites

- **Rust**: Edition 2024 (requires recent Rust toolchain)
- **Node.js & NPM**: For MCP Inspector testing tool
- **Local Dependencies**: Requires `../fhirpath-rs` in the parent directory

### Development Dependencies Setup

The project uses **local development dependencies** from `../fhirpath-rs`:
```toml
octofhir-fhirpath = { path = "../fhirpath-rs/crates/octofhir-fhirpath" }
octofhir-fhirpath-diagnostics = { path = "../fhirpath-rs/crates/fhirpath-diagnostics" }
```

For production builds, switch to published crates (commented sections in Cargo.toml).

### Quick Setup

```bash
# Check development environment
just dev-setup-check

# Install development tools
just install-tools

# Initial build and test
just quickstart
```

### Build Commands

```bash
# Development build
just build
cargo build

# Release build (with optimizations)
just build-release
cargo build --release

# Fast check without building
just check
cargo check

# Clean build artifacts
just clean
```

### Multiple Binaries

The project provides three binaries:
- **octofhir-mcp**: Main MCP server
- **benchmark**: Performance testing
- **validate-server**: Server validation tool

```bash
# Verify all binaries build
just verify-binaries

# Run specific binary
cargo run --bin octofhir-mcp
cargo run --bin benchmark
cargo run --bin validate-server
```

### Development Profile

The project uses optimized dev profile for better performance:
```toml
[profile.dev]
opt-level = 1
```

## Testing Information

### Test Framework Configuration

Uses multiple testing frameworks:
- **Standard Rust tests**: `#[test]` and `#[cfg(test)]`
- **Async tests**: `tokio-test` with `#[tokio::test]`
- **Property testing**: `proptest`
- **Parameterized tests**: `rstest`
- **Benchmarking**: `criterion`

### Running Tests

```bash
# Run all tests
just test
cargo test

# Run specific test
just test-specific test_name
cargo test test_name

# Run with output visible
cargo test -- --nocapture

# Run tests with coverage (requires cargo-tarpaulin)
just test-coverage
```

### Test Organization

Tests are organized in several ways:
1. **Inline tests**: In the same file as implementation (`#[cfg(test)]` modules)
2. **Separate test files**: Like `src/tools/fhirpath_evaluate_test.rs`
3. **Integration tests**: In `tests/` directory

### Test Example Patterns

#### Basic Test Structure
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_functionality() {
        // Synchronous test
        assert_eq!(2 + 2, 4);
    }

    #[tokio::test]
    async fn test_async_functionality() {
        let tool = FhirPathEvaluateTool::new().unwrap();
        
        let params = json!({
            "expression": "Patient.name.given",
            "resource": {
                "resourceType": "Patient",
                "name": [{"given": ["John"], "family": "Doe"}]
            }
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
    }
}
```

#### Error Handling Tests
```rust
#[tokio::test]
async fn test_error_conditions() {
    let tool = FhirPathEvaluateTool::new().unwrap();
    
    let params = json!({
        "expression": "",  // Empty expression should fail
        "resource": {"resourceType": "Patient"}
    });

    let result = tool.execute(params).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}
```

### Server Testing with MCP Inspector

The project includes sophisticated server testing capabilities:

```bash
# Start server with HTTP transport
just serve-sse 3005

# Test all server endpoints
just test-server 3005

# Complete testing workflow
just test-complete 3005

# Use MCP Inspector for interactive testing
just inspector 3005
```

#### Manual API Testing
```bash
# Health check
curl http://localhost:3005/health

# List available tools
curl http://localhost:3005/mcp/tools/list

# Test FHIRPath evaluation
curl -X POST -H "Content-Type: application/json" \
  -d '{"arguments": {"expression": "Patient.name.family", "resource": {"resourceType": "Patient", "name": [{"family": "Doe", "given": ["John"]}]}}}' \
  http://localhost:3005/mcp/tools/fhirpath_evaluate
```

## Development Information

### Code Style and Formatting

```bash
# Format code
just fmt
cargo fmt

# Check formatting
just fmt-check

# Lint code
just lint
cargo clippy

# Fix linting issues
just lint-fix
cargo clippy --fix

# Pedantic linting
just lint-pedantic
```

### Development Workflow Commands

```bash
# Complete development cycle
just dev          # Format, check, test
just ci           # Format check, lint, test
just full         # Clean, format, lint, test, docs

# Watch for changes
just watch        # Auto-run checks on file changes
```

### Project Architecture

#### Module Structure
- **server**: Core MCP server implementation
- **transport**: HTTP, STDIO, WebSocket transports
- **tools**: FHIRPath evaluation and other MCP tools
- **resources**: FHIR resource handling
- **prompts**: MCP prompt templates
- **security**: Authentication and security features
- **cache**: Caching mechanisms
- **metrics**: Performance monitoring
- **config**: Configuration management
- **fhirpath_engine**: Shared FHIRPath engine factory

#### Key Design Patterns

1. **Async-first architecture**: Uses tokio extensively
2. **Tool trait pattern**: All MCP tools implement common `Tool` trait
3. **Factory pattern**: FHIRPath engine uses factory pattern for shared instances
4. **Error handling**: Consistent use of `anyhow` and `thiserror`
5. **Serialization**: Heavy use of `serde` for JSON handling

### Serde Usage Patterns

When creating response structures, ensure both Serialize and Deserialize are implemented:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseStruct {
    pub field: String,
    // All nested structs also need Serialize + Deserialize
}
```

### Local Development Considerations

- **Path dependencies**: Uses local `../fhirpath-rs` for development
- **Feature flags**: Multiple transport and security features available
- **Profile optimization**: Release builds use LTO and single codegen unit
- **Mock providers**: Uses `MockModelProvider` for testing

### Performance and Monitoring

```bash
# Generate metrics
just metrics

# Profile application (requires perf on Linux)
just profile

# Generate flamegraph (requires cargo-flamegraph)
just flamegraph
```

### Documentation

```bash
# Build and open documentation
just docs
cargo doc --open

# Build documentation only
just docs-build
```

### Dependency Management

```bash
# Update dependencies
just update
cargo update

# Check for outdated dependencies
just outdated
cargo outdated

# Security audit
just audit
cargo audit
```

## Project-Specific Notes

### Transport Protocols
- **STDIO**: For command-line MCP clients
- **HTTP**: REST API with SSE support for web clients
- **WebSocket**: Real-time communication (feature-gated)

### FHIRPath Integration
- Uses local `octofhir-fhirpath` crates for development
- Provides comprehensive FHIRPath evaluation with diagnostics
- Supports context variables and timeout handling
- Performance metrics included in responses

### MCP Inspector Integration
The project has excellent integration with the MCP Inspector for testing:
- Automatic server startup and connection
- Pre-configured endpoints for easy testing
- SSE support for real-time communication

### Key Configuration Files
- **Cargo.toml**: Dependencies and build configuration
- **justfile**: Development workflow commands (280 lines of comprehensive tooling)
- **src/lib.rs**: Module structure and public API

This project follows modern Rust development practices with sophisticated tooling for both development and production use cases.
