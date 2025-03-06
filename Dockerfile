# Build stage
FROM rust:1.72-slim as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/
COPY tests/ ./tests/

# Build dependencies - this is the caching layer
RUN mkdir -p ./src/bin && \
    echo "fn main() {}" > ./src/bin/dummy.rs && \
    cargo build --release --bin dummy && \
    rm -rf ./src/bin/dummy.rs

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/amazon-rose-forest /usr/local/bin/

# Set environment variables
ENV RUST_LOG=info

# Expose the port the server listens on
EXPOSE 8000

# Run the binary
CMD ["amazon-rose-forest", "serve"]