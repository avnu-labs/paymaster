FROM rust:1.87 as builder

ARG BUILD_TOKEN

WORKDIR /paymaster

COPY ./ .

# Build for release.
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12 AS distroless
COPY --from=builder /paymaster/target/release/paymaster-service .
CMD ["./paymaster-service"]

FROM debian:12-slim AS debian
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash jq curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /paymaster/target/release/paymaster-service .
CMD ["./paymaster-service"]
