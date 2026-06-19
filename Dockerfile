FROM rust:1.82-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release --bin farga-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/farga-server /usr/local/bin/farga-server
# Seed docs are copied to /app/docs; initContainer seeds PVC on first run.
COPY docs/ /app/docs/
EXPOSE 7500
ENTRYPOINT ["farga-server"]
