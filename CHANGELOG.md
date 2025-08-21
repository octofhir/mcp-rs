# Changelog

All notable changes to the OctoFHIR MCP Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial implementation of OctoFHIR MCP Server
- FHIRPath evaluation tools through Model Context Protocol
- Support for stdio and HTTP transports
- Four core tools: `fhirpath_evaluate`, `fhirpath_parse`, `fhirpath_extract`, `fhirpath_analyze`
- Docker containerization support
- GitHub Actions for automated releases
- Comprehensive documentation and examples
- Development tooling with Just commands
- MCP Inspector integration for testing

### Infrastructure
- Multi-platform binary releases (Linux, macOS, Windows)
- Docker images published to GitHub Container Registry
- Automated version bumping and changelog generation
- Cross-compilation support for ARM64 and x86_64

## [0.1.0] - 2024-XX-XX

### Added
- Initial release of OctoFHIR MCP Server
- Base functionality for FHIRPath evaluation through MCP
- Documentation and setup instructions