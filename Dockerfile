# syntax=docker/dockerfile:1
FROM rust:1.97-bookworm AS builder
RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y  \
    musl-tools  \
    perl \
    make \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm src/main.rs

COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM gcr.io/distroless/static-debian12
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/git_kord /usr/local/bin/git_kord
CMD ["git_kord"]