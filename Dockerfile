FROM rust:latest AS builder

WORKDIR /app

COPY ethereum-node-healthcheck-src .

RUN cargo build --release

FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

# These packages are needed to make https requests
RUN apt update && \
    apt install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ethereum-node-healthcheck /usr/local/bin/ethereum-node-healthcheck

CMD ["ethereum-node-healthcheck"]
