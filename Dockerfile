# Multi-stage build for OctoFHIR MCP Server
FROM rust:1.88-slim AS planner
WORKDIR /app
# Install cargo-chef for dependency caching
RUN cargo install cargo-chef

# Copy only the files needed for dependency resolution
COPY Cargo.toml Cargo.lock ./
RUN cargo chef prepare --recipe-path recipe.json

FROM rust:1.88-slim AS cacher
WORKDIR /app
RUN cargo install cargo-chef

# Install required system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the recipe and build dependencies
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies with cargo-chef
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.88-slim AS builder
WORKDIR /app

# Install required system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy pre-built dependencies from cacher stage
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# Copy source code and build configuration
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build only the application (dependencies already cached)
RUN cargo build --release --bin octofhir-mcp

# Runtime stage  
FROM debian:bookworm-slim AS runtime

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