# Use the official Rust image.
# https://hub.docker.com/_/rust
# FROM rust:latest

# # Copy local code to the container image.
# WORKDIR /usr/cloak/
# COPY ./src ./src
# COPY ./Cargo.lock .
# COPY ./Cargo.toml .

# # Install production dependencies and build a release artifact.
# RUN cargo install --path .

# # Run the web service on container startup.
# CMD ["invisibility"]

# FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
# WORKDIR /app

# FROM chef AS planner
# COPY . .
# RUN cargo chef prepare --recipe-path recipe.json

# FROM chef AS builder 
# COPY --from=planner /app/recipe.json recipe.json
# # Build dependencies - this is the caching Docker layer!
# RUN cargo chef cook --release --recipe-path recipe.json
# # Build application
# COPY . .
# RUN cargo build --release --bin invisibility

# We do not need the Rust toolchain to run the binary!
# FROM debian:bookworm-slim AS runtime
# FROM rust:latest AS
FROM lukemathwalker/cargo-chef:latest-rust-1
WORKDIR /app
RUN apt-get update && apt install -y openssl
RUN cargo install cargo-shuttle
COPY . .
# COPY --from=builder /app/target/release/invisibility /usr/local/bin
# ENTRYPOINT ["/usr/local/bin/invisibility"]
ENTRYPOINT ["cargo"]

CMD ["shuttle", "run", "--release", "--port", "8000", "--external"]
