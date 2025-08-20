# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **dedicated repository** for the OctoFHIR Model Context Protocol (MCP) server in Rust. The project exposes high-performance FHIRPath evaluation and other FHIR tools through standardized MCP interfaces, making OctoFHIR functionality accessible to AI assistants and other MCP clients.

**Repository Structure**: This is a standalone repository (`octofhir/mcp-rs`) that depends on the main FHIRPath library located at `../fhirpath-rs` for local development.

## Architecture Design

Based on ADR-001, this project follows a modular, extensible architecture:

### Core Structure
```
mcp-rs/                      # Dedicated MCP server repository
├── Cargo.toml               # Project configuration with local dependencies
├── justfile                 # Development command runner
├── src/
│   ├── lib.rs              # Public API for library usage
│   ├── server.rs           # Core MCP server implementation using rmcp SDK
│   ├── transport.rs        # Multi-transport support (stdio, HTTP/SSE)
│   ├── tools.rs            # MCP tools implementation (fhirpath_*)
│   ├── fhirpath_engine.rs  # FHIRPath engine factory and management
│   ├── resources/          # MCP resources (schemas, examples, docs)
│   ├── prompts/            # MCP prompts for common patterns
│   ├── security/           # Authentication, validation, CORS
│   ├── cache/              # Performance optimization layers
│   ├── metrics/            # Health checks and monitoring
│   ├── config/             # Configuration management
│   └── bin/                # Binary executables
│       ├── octofhir-mcp.rs     # Main server binary
│       ├── benchmark.rs        # Performance benchmarking
│       └── validate-server.rs  # Server validation tool
├── tests/                   # Integration and unit tests
│   ├── integration_sdk.rs   # SDK integration tests
│   ├── fixtures/           # Test data (FHIR resources, expressions)
│   └── common/             # Test utilities
└── ../fhirpath-rs/          # Parent directory contains FHIRPath library
```

### Tool Naming Convention
- All tools use descriptive prefixes: `fhirpath_`, `terminology_`, `validation_`, etc.
- **Current implementation**: `fhirpath_evaluate`, `fhirpath_parse`, `fhirpath_extract`, `fhirpath_analyze`
- Future expansion: `terminology_*`, `validation_*`, `conversion_*`, `bundle_*` tools

## MCP Implementation

**CRITICAL**: For MCP protocol implementation, we MUST ALWAYS use the official MCP Rust SDK:
- Repository: https://github.com/modelcontextprotocol/rust-sdk
- **ALWAYS use `rmcp` crate with "server" feature** for MCP server implementation
- **ALWAYS prefer rmcp SDK** over any other server implementation approaches
- Use `rmcp-macros` for procedural macros and tool generation
- Follow official MCP specification and RMCP SDK examples exclusively
- Current version: rmcp v0.6.0 (with transport features)
- **Never implement custom MCP protocol handling** - use the SDK

## Development Commands

This is a Rust project using Cargo for build management with Just for convenient task automation:

### Quick Start Commands (using Just)
```bash
# Quick development workflow
just quickstart           # Complete setup check and demo

# Server commands
just stdio                # Start MCP server with stdio transport
just http [PORT]          # Start MCP server with HTTP transport (default: 3005)
just demo                 # Run FHIRPath evaluation demo
just info                 # Show server information

# Development commands
just build                # Build the project
just test                 # Run all tests
just dev                  # Format, check, and test
just fmt                  # Format code
just lint                 # Run clippy linter
just docs                 # Build and open documentation

# Testing commands
just test-complete        # Complete testing workflow
just test-integration     # Run integration tests
just inspector [PORT]     # Start server with MCP Inspector for testing
```

### Direct Cargo Commands
```bash
# Build the project
cargo build
cargo build --release

# Run tests
cargo test
cargo test test_name
cargo test --test integration_sdk

# Check code without building
cargo check

# Format and lint
cargo fmt
cargo clippy

# Build documentation
cargo doc --open

# Run binaries
cargo run --bin octofhir-mcp stdio
cargo run --bin octofhir-mcp http --port 3005
cargo run --bin benchmark
cargo run --bin validate-server
```

## Key Implementation Goals

### Performance Architecture
- Intelligent multi-level caching (expression compilation, resource validation, schema)
- Arena-based memory management with automatic cleanup
- Streaming support for large resources
- Connection pooling for HTTP transport

### Security Architecture  
- Multi-factor authentication (JWT, API keys, OAuth 2.0/OIDC)
- Role-based access control (RBAC)
- Comprehensive input validation and FHIRPath expression sanitization
- Audit logging with complete request/response trails

### Transport Support
- **Stdio transport**: Local CLI integration
- **HTTP/SSE transport**: Web applications and remote access
- **WebSocket transport**: Real-time applications (future)

### Local Development Setup

**Directory Structure**:
```
parent-directory/
├── fhirpath-rs/             # Main FHIRPath library repository
└── mcp-rs/                  # This MCP server repository
```

**Dependencies**:
- **Local Development**: Uses `../fhirpath-rs` path dependencies
- **Production**: Switches to published crate dependencies
- Core integration: `octofhir-fhirpath`, `fhirpath-diagnostics`, `fhirpath-analyzer`, `fhirpath-tools`

**Switching Dependencies**:
```bash
# For local development (default)
cargo build  # Uses ../fhirpath-rs

# For production release
# Update Cargo.toml to use published crates:
# octofhir-fhirpath = "0.1"
```

## Development Phases

The project follows a phased implementation approach:

1. **Core Foundation**: MCP server with stdio/HTTP transports, basic FHIRPath tools
2. **Essential Tooling**: Complete FHIRPath functionality, caching, diagnostics  
3. **Production Features**: Security, resources, prompts, observability
4. **Distribution**: Cross-platform binaries, Docker images, integration testing
5. **Ecosystem Growth**: Advanced features, community feedback, optimization

## Testing Strategy

When writing tests:
- Use `cargo test` to run the full test suite or `just test` for convenience
- Use `cargo test --test integration_sdk` for SDK integration tests
- Use `just test-complete` for comprehensive testing workflow
- Write integration tests for MCP protocol compliance using rmcp SDK
- Include performance benchmarks for FHIRPath operations
- Test multi-transport functionality across stdio and HTTP
- Use `just inspector` to test with MCP Inspector tool
- Validate security features with comprehensive test scenarios

## Local Development Workflow

**Setup**:
1. Clone both repositories in the same parent directory:
   ```bash
   git clone https://github.com/octofhir/fhirpath-rs.git
   git clone https://github.com/octofhir/mcp-rs.git
   ```

2. The MCP server automatically uses local FHIRPath dependencies via Cargo.toml path references

**Development Commands**:
```bash
# Ensure FHIRPath library builds first
cd ../fhirpath-rs && cargo build

# Build MCP server with local dependencies
cd ../mcp-rs && cargo build

# Run tests against local FHIRPath implementation
cargo test
# OR use justfile commands for convenience
just test

# Run the MCP server locally  
just stdio              # stdio transport (recommended for MCP clients)
just http 3005          # HTTP transport on port 3005
cargo run --bin octofhir-mcp stdio    # direct cargo command
```

## Configuration Management

The server supports multiple configuration methods:
- Command-line arguments for runtime settings
- Environment variables for deployment configurations  
- Configuration files for complex setups
- Default configurations optimized for development vs production

## Distribution Strategy

**Dedicated Repository Benefits**:
- Independent versioning and release cycles
- Focused development without workspace complexity
- Easier distribution and deployment
- Clear separation of concerns

**Release Process**:
- **GitHub Releases**: Cross-platform binaries for Linux, Windows, macOS
- **Docker Images**: Multi-architecture support via GitHub Container Registry
- **Crate Registry**: Published as `octofhir-mcp` crate
- **Local Development**: Uses `../fhirpath-rs` for active development

## Observability

The project includes comprehensive observability features:
- OpenTelemetry integration for distributed tracing
- Health check endpoints for monitoring
- Performance metrics and request analytics
- Structured error reporting with context



## Guidelines

Apply the following guidelines when developing octofhir-mcp:
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Rust Style Guide](https://rust-lang.github.io/rust-style-guide/)


## Development Process

### Architecture Decision Records (ADRs)
Before implementing major features:
1. Create ADR following: https://github.com/joelparkerhenderson/architecture-decision-record
2. Split implementation into phases/tasks stored in `tasks/` directory
3. Update task files with implementation status

### Task Management
For every ADR implementation split record into phases/tasks and store in `tasks/` directory. Maintain a specific task file when working on it. Before starting on the first task, create all tasks for future use. After implementing features from a task file update its status.

### Debug Workflow
For debugging cases create a simple test inside the test directory and delete it after resolving the issue.

