FROM rust:1.85 AS builder

WORKDIR /build
RUN cargo install --root /build starknet-devnet@0.4.2

FROM gcr.io/distroless/cc-debian12

WORKDIR /app

COPY --from=builder /build/bin/starknet-devnet /app/starknet-devnet
COPY blockchain.json /app/blockchain.json

CMD ["/app/starknet-devnet", "--host=0.0.0.0", "--seed=0", "--dump-path=/app/blockchain.json"]

