# Build on the target platform (native ARM64 or buildx/QEMU).
FROM rust:1.99.1-trixie AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/lanx-api /usr/local/bin/lanx-api
USER 65532:65532
ENV RUST_LOG=info
EXPOSE 8383
ENTRYPOINT ["/usr/local/bin/lanx-api"]
