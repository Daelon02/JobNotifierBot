# Stage 1: Build
FROM rust:1.95-slim-bookworm AS builder

WORKDIR /usr/src/app

# Install build dependencies (pkg-config, OpenSSL, and libpq for PostgreSQL)
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy src files to cache external dependencies compilation
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release
RUN rm -rf target/release/deps/job_notifier_bot* target/release/deps/libjob_notifier_bot* target/release/.fingerprint/job_notifier_bot* target/release/job_notifier_bot*

# Copy actual source code and database migrations
COPY src ./src
COPY migrations ./migrations

# Build the application
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install ca-certificates and shared runtime libraries
RUN apt-get update && apt-get install -y ca-certificates libssl3 libpq5 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/job_notifier_bot /app/

# Ensure local storage directory exists for resumes
RUN mkdir -p /app/storage/resumes

# Run binary with config.yaml
ENTRYPOINT ["/app/job_notifier_bot", "/app/config.yaml"]
