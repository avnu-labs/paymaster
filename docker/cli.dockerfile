FROM rust:1.83 as builder

WORKDIR /paymaster

COPY . .

# Build for release.
RUN cargo build --release --bin paymaster-cli

FROM gcr.io/distroless/cc-debian12 AS distroless
COPY --from=builder /paymaster/target/release/paymaster-cli .
CMD ["./paymaster-cli"]

FROM debian:12-slim AS debian
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash jq curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /paymaster/target/release/paymaster-cli .
CMD ["./paymaster-cli"]

