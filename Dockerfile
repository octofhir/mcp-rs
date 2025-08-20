# Multi-stage build for OctoFHIR MCP Server
FROM rust:1.88-slim AS builder

# Install required system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs and bin directory to build dependencies
RUN mkdir -p src/bin && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/octofhir-mcp.rs

# Build dependencies (this layer will be cached unless Cargo.toml changes)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release --bin octofhir-mcp

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r octofhir && useradd -r -g octofhir octofhir

# Create app directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/octofhir-mcp /usr/local/bin/octofhir-mcp

# Make binary executable
RUN chmod +x /usr/local/bin/octofhir-mcp

# Change ownership to non-root user
RUN chown -R octofhir:octofhir /app

# Switch to non-root user
USER octofhir

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD octofhir-mcp info || exit 1

# Expose default HTTP port
EXPOSE 3005

# Set default environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Default command (stdio transport)
CMD ["octofhir-mcp", "stdio"]

# Labels for metadata
LABEL org.opencontainers.image.title="OctoFHIR MCP Server"
LABEL org.opencontainers.image.description="High-performance FHIRPath evaluation and FHIR tooling through Model Context Protocol"
LABEL org.opencontainers.image.vendor="OctoFHIR Team"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"
LABEL org.opencontainers.image.source="https://github.com/octofhir/mcp-rs"
LABEL org.opencontainers.image.documentation="https://github.com/octofhir/mcp-rs/blob/main/README.md"