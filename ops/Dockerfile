FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/askama.toml ./server/

# Cache dependencies by building an empty crate first.
RUN mkdir -p server/src && echo 'fn main() {}' > server/src/main.rs
RUN cargo build --release --bin stage-server 2>/dev/null || true
RUN rm -f target/release/deps/stage_server*

# Build the real binary.
COPY server/src ./server/src
COPY web/templates ./web/templates
RUN cargo build --release --bin stage-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/stage-server ./stage-server
COPY web/static ./web/static

ENV PORT=8080
ENV RUST_LOG=info,stage_server=info

EXPOSE 8080

CMD ["./stage-server"]
