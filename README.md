# OctoFHIR MCP Server

> **High-performance FHIRPath evaluation and FHIR tooling through Model Context Protocol**

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/octofhir/mcp-rs)
[![Rust](https://img.shields.io/badge/rust-1.88+-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-2025--06--18-green.svg)](https://modelcontextprotocol.io)
[![CI](https://github.com/octofhir/mcp-rs/workflows/Code%20Quality/badge.svg)](https://github.com/octofhir/mcp-rs/actions)
[![Security](https://github.com/octofhir/mcp-rs/workflows/Security%20Scan/badge.svg)](https://github.com/octofhir/mcp-rs/actions)
[![Support](https://img.shields.io/badge/Support-Boosty-orange.svg)](https://boosty.to/octoshikari)

The **OctoFHIR MCP Server** provides AI assistants and applications with powerful FHIR capabilities through the standardized Model Context Protocol (MCP). Built by the OctoFHIR Team, this server exposes high-performance FHIRPath evaluation and other FHIR tools to make healthcare data processing accessible to AI systems.

## 🚀 Quick Start

### Prerequisites

- **Rust 1.88+** (for building from source)
- **Node.js 20+** (for MCP Inspector testing)
- **Git** (for cloning the repository)

### Installation

#### Option 1: Download Pre-built Binary (Recommended)

```bash
# Download latest release for your platform
curl -L https://github.com/octofhir/mcp-rs/releases/latest/download/octofhir-mcp-linux-x86_64.tar.gz | tar xz
# Or for macOS:
curl -L https://github.com/octofhir/mcp-rs/releases/latest/download/octofhir-mcp-macos-x86_64.tar.gz | tar xz
# Or for Windows:
# Download octofhir-mcp-windows-x86_64.zip from releases page

# Make executable and move to PATH
chmod +x octofhir-mcp
sudo mv octofhir-mcp /usr/local/bin/
```

#### Option 2: Build from Source

```bash
# Clone repository
git clone https://github.com/octofhir/mcp-rs.git
cd mcp-rs

# Build release binary
cargo build --release

# Binary will be at target/release/octofhir-mcp
```

#### Option 3: Docker

```bash
# Pull from GitHub Container Registry
docker pull ghcr.io/octofhir/octofhir-mcp:latest

# Run with stdio transport
docker run -i ghcr.io/octofhir/octofhir-mcp:latest stdio

# Run with HTTP transport
docker run -p 3005:3005 ghcr.io/octofhir/octofhir-mcp:latest http --host 0.0.0.0 --port 3005
```

### Basic Usage

#### Start the MCP Server

```bash
# For MCP clients (recommended)
octofhir-mcp stdio

# For web applications (HTTP transport)
octofhir-mcp http --port 3005

# With debug logging
RUST_LOG=debug octofhir-mcp stdio
```

#### Test with Demo

```bash
# Run built-in FHIRPath evaluation demo
octofhir-mcp demo
```

## 📋 Available Tools

The OctoFHIR MCP Server provides four powerful FHIRPath tools:

### 1. `fhirpath_evaluate`
Evaluate FHIRPath expressions against FHIR resources with performance metrics.

**Example:**
```json
{
  "expression": "Patient.name.given",
  "resource": {
    "resourceType": "Patient",
    "name": [{"given": ["John"], "family": "Doe"}]
  }
}
```

**Response:**
```json
{
  "results": [["John"]],
  "result_count": 1,
  "evaluation_time_ms": 2.5,
  "expression_complexity": "Simple"
}
```

### 2. `fhirpath_parse`
Parse and validate FHIRPath expressions with detailed syntax analysis.

**Example:**
```json
{
  "expression": "Patient.name.where(use = 'official').family",
  "include_ast": true
}
```

### 3. `fhirpath_extract`
Extract data from FHIR resources using FHIRPath with flexible formatting options.

**Example:**
```json
{
  "expressions": {
    "patient_name": "Patient.name.family",
    "birth_date": "Patient.birthDate"
  },
  "resource": {
    "resourceType": "Patient",
    "name": [{"family": "Smith"}],
    "birthDate": "1990-01-01"
  },
  "format": "json"
}
```

### 4. `fhirpath_analyze`
Analyze FHIRPath expressions providing detailed information about syntax, performance, and usage patterns.

## 🔧 Integration Guide

### With Claude Desktop

Add to your Claude Desktop configuration (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "octofhir": {
      "command": "octofhir-mcp",
      "args": ["stdio"]
    }
  }
}
```

### With MCP Inspector (Development)

```bash
# Install MCP Inspector
npm install -g @modelcontextprotocol/inspector

# Start server and inspector
octofhir-mcp http --port 3005 &
npx @modelcontextprotocol/inspector
# Configure: Transport: HTTP Streamable, URL: http://localhost:3005
```

### With Custom MCP Client

```typescript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

// Connect via stdio
const transport = new StdioClientTransport({
  command: 'octofhir-mcp',
  args: ['stdio']
});

const client = new Client({
  name: "my-fhir-app",
  version: "1.0.0"
}, {
  capabilities: {}
});

await client.connect(transport);

// Use FHIRPath evaluation
const result = await client.callTool({
  name: "fhirpath_evaluate",
  arguments: {
    expression: "Patient.name.family",
    resource: { /* your FHIR resource */ }
  }
});
```

### With HTTP API

```bash
# Start HTTP server
octofhir-mcp http --port 3005

# Make requests
curl -X POST http://localhost:3005/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "fhirpath_evaluate",
      "arguments": {
        "expression": "Patient.active",
        "resource": {"resourceType": "Patient", "active": true}
      }
    }
  }'
```

## 🛠️ Development

### Development Setup

```bash
# Clone with local dependencies
git clone https://github.com/octofhir/fhirpath-rs.git
git clone https://github.com/octofhir/mcp-rs.git
cd mcp-rs

# Install development tools
cargo install just
npm install -g @modelcontextprotocol/inspector

# Quick start
just quickstart
```

### Development Commands

```bash
# Start development server
just stdio                # stdio transport
just http 3005            # HTTP transport

# Development workflow
just dev                  # format, check, test
just test-complete        # comprehensive testing
just inspector            # test with MCP Inspector

# Build and release
just build                # debug build
just build-release        # optimized build
```

## 📚 API Reference

### Command Line Interface

```bash
octofhir-mcp [TRANSPORT] [OPTIONS]

TRANSPORTS:
    stdio                   Start with stdio transport (for MCP clients)
    http                    Start with HTTP transport (for web applications)
    demo                    Run FHIRPath evaluation demonstration
    info                    Show server information
    validate                Validate server configuration

HTTP OPTIONS:
    --host <HOST>           Host to bind to [default: 127.0.0.1]
    --port <PORT>           Port to bind to [default: 3005]
    --log-level <LEVEL>     Log level [default: info]

GLOBAL OPTIONS:
    -h, --help              Print help information
    -V, --version           Print version information
```

### Environment Variables

```bash
RUST_LOG=debug              # Enable debug logging
OCTOFHIR_CACHE_SIZE=1000     # Set cache size
OCTOFHIR_TIMEOUT_MS=30000    # Set evaluation timeout
```

## 🔍 Examples

### Basic FHIRPath Evaluation

```bash
# Using command line demo
octofhir-mcp demo

# Using MCP Inspector
octofhir-mcp http --port 3005
# Open MCP Inspector and connect to http://localhost:3005
```

### Complex FHIRPath Queries

```json
{
  "tool": "fhirpath_evaluate",
  "arguments": {
    "expression": "Bundle.entry.resource.where(resourceType = 'Patient').name.where(use = 'official').family",
    "resource": {
      "resourceType": "Bundle",
      "entry": [
        {
          "resource": {
            "resourceType": "Patient",
            "name": [
              {"use": "official", "family": "Smith", "given": ["John"]},
              {"use": "maiden", "family": "Johnson"}
            ]
          }
        }
      ]
    }
  }
}
```

### Batch Data Extraction

```json
{
  "tool": "fhirpath_extract",
  "arguments": {
    "expressions": {
      "patient_id": "Patient.id",
      "full_name": "Patient.name.where(use = 'official').family + ', ' + Patient.name.where(use = 'official').given.join(' ')",
      "active_status": "Patient.active",
      "birth_year": "Patient.birthDate.substring(0, 4)"
    },
    "resource": {
      "resourceType": "Patient",
      "id": "example-patient",
      "active": true,
      "name": [{"use": "official", "family": "Doe", "given": ["Jane", "Marie"]}],
      "birthDate": "1985-03-15"
    },
    "format": "json"
  }
}
```

## 🐳 Docker Usage

### Basic Docker Commands

```bash
# Pull latest image
docker pull ghcr.io/octofhir/octofhir-mcp:latest

# Run with stdio (for MCP clients)
docker run -i ghcr.io/octofhir/octofhir-mcp:latest stdio

# Run with HTTP server
docker run -p 3005:3005 ghcr.io/octofhir/octofhir-mcp:latest http --host 0.0.0.0 --port 3005

# Run demo
docker run ghcr.io/octofhir/octofhir-mcp:latest demo

# With custom configuration
docker run -e RUST_LOG=debug -p 3005:3005 ghcr.io/octofhir/octofhir-mcp:latest http --host 0.0.0.0 --port 3005
```

### Docker Compose

```yaml
version: '3.8'
services:
  octofhir-mcp:
    image: ghcr.io/octofhir/octofhir-mcp:latest
    ports:
      - "3005:3005"
    environment:
      - RUST_LOG=info
    command: ["http", "--host", "0.0.0.0", "--port", "3005"]
    restart: unless-stopped
```

## 🚀 Performance

The OctoFHIR MCP Server is built for high performance:

- **Expression Compilation Caching**: Compiled FHIRPath expressions are cached for repeated use
- **Memory Management**: Arena-based allocation for optimal memory usage
- **Concurrent Processing**: Async runtime for handling multiple requests
- **Optimized Builds**: Release builds with LTO and single codegen unit

**Benchmarks** (on typical hardware):
- Simple expressions: ~0.1-1ms evaluation time
- Complex expressions: ~1-10ms evaluation time  
- Large resources (100KB+): ~10-50ms evaluation time

## 🔒 Security

- **Input Validation**: All FHIRPath expressions and FHIR resources are validated
- **Expression Sanitization**: Dangerous operations are filtered out
- **Resource Limits**: Configurable timeouts and memory limits
- **Audit Logging**: Comprehensive request/response logging available

## 🤝 Contributing

We welcome contributions to the OctoFHIR MCP Server! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Process

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run `just dev` to verify quality
5. Submit a pull request

## 📄 License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## 🏥 About OctoFHIR

The OctoFHIR MCP Server is part of the **OctoFHIR ecosystem** - a comprehensive suite of tools for FHIR data processing and healthcare interoperability.

**Other OctoFHIR Projects:**
- [octofhir/fhirpath-rs](https://github.com/octofhir/fhirpath-rs) - High-performance FHIRPath engine
- [octofhir/fhir-tools](https://github.com/octofhir/fhir-tools) - FHIR validation and conversion tools

## 📞 Support

- **Documentation**: [OctoFHIR Docs](https://docs.octofhir.org)
- **Issues**: [GitHub Issues](https://github.com/octofhir/mcp-rs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/octofhir/mcp-rs/discussions)
- **Support Development**: [Boosty](https://boosty.to/octoshikari) 💖
- **Email**: support@octofhir.org

## 🎯 Roadmap

- **Q3 2025**: WebSocket transport support and advanced caching optimizations
- **Q4 2025**: FHIR terminology services and validation tools
- **Q1 2026**: Bundle processing, batch operations, and streaming support
- **Q2 2026**: Advanced security features and enterprise integrations

---

Made with ❤️ by the **OctoFHIR Team**