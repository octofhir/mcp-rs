# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **dedicated repository** for the OctoFHIR Model Context Protocol (MCP) server in Rust. The project exposes high-performance FHIRPath evaluation and other FHIR tools through standardized MCP interfaces, making OctoFHIR functionality accessible to AI assistants and other MCP clients.

**Repository Structure**: This is a standalone repository (`octofhir/mcp-rs`) that depends on the main FHIRPath library located at `../fhirpath-rs` for local development.

## Architecture Design
update
Based on ADR-001, this project follows a modular, extensible architecture:

### Core Structure
```
mcp-rs/                      # Dedicated MCP server repository
├── Cargo.toml               # Project configuration with local dependencies
├── src/
│   ├── lib.rs              # Public API for library usage
│   ├── server.rs           # Core MCP server implementation  
│   ├── transport/          # Multi-transport support (stdio, HTTP/SSE, WebSocket)
│   ├── tools/              # MCP tools (fhirpath_*, terminology_*, etc.)
│   ├── resources/          # MCP resources (schemas, examples, docs)
│   ├── prompts/            # MCP prompts for common patterns
│   ├── security/           # Authentication, rate limiting, CORS
│   ├── cache/              # Performance optimization layers
│   ├── metrics/            # OpenTelemetry integration
│   ├── config/             # Advanced configuration management
│   └── bin/                # Binary executables
│       ├── octofhir-mcp.rs     # Main server binary
│       ├── benchmark.rs        # Performance benchmarking
│       └── validate-server.rs  # Server validation tool
└── ../fhirpath-rs/          # Parent directory contains FHIRPath library
```

### Tool Naming Convention
- All tools use descriptive prefixes: `fhirpath_`, `terminology_`, `validation_`, etc.
- Current focus: `fhirpath_evaluate`, `fhirpath_parse`, `fhirpath_extract`, `fhirpath_explain`
- Future expansion: `terminology_*`, `validation_*`, `conversion_*`, `bundle_*` tools

## MCP Implementation

**IMPORTANT**: For MCP protocol implementation, we MUST use the official MCP Rust SDK:
- Repository: https://github.com/modelcontextprotocol/rust-sdk
- Use `rmcp` crate with "server" feature for MCP server implementation
- Use `rmcp-macros` for procedural macros and tool generation
- Follow official MCP specification and RMCP SDK examples
- Current version: rmcp v0.5.0

## Development Commands

This is a Rust project using Cargo for build management:

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test test_name

# Check code without building
cargo check

# Format code  
cargo fmt

# Run linter
cargo clippy

# Build documentation
cargo doc --open

# Run the MCP server binary
cargo run --bin octofhir-mcp

# Run benchmarks
cargo run --bin benchmark

# Validate server configuration
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
- Use `cargo test` to run the full test suite
- Write integration tests for MCP protocol compliance
- Include performance benchmarks for FHIRPath operations
- Test multi-transport functionality across stdio and HTTP
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

# Run the MCP server locally
cargo run --bin octofhir-mcp
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

Apply the following guidelines when developing fhirpath-core:
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

