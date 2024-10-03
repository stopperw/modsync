FROM rust:latest AS builder
WORKDIR /modsync
COPY . .
# RUN cargo install sqlx-cli
ENV SQLX_OFFLINE=true
# RUN cargo sqlx prepare --workspace --check && \
# 	cargo build --release --bin modsync_server
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/usr/local/cargo/git/db \
	--mount=type=cache,target=/modsync/target \
	cargo build --release --bin modsync_server && \
	cp /modsync/target/release/modsync_server /modsync/server

FROM debian:bookworm-slim
WORKDIR /modsync
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates
COPY --from=builder \
	/modsync/server /modsync/server

EXPOSE 7040/tcp
ENV RUST_LOG=info
CMD ["/modsync/server"]

