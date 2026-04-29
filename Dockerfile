# syntax=docker/dockerfile:1.23
FROM rust:1.95-trixie AS builder

WORKDIR /usr/src/rover

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/rover/target \
    cargo build --release --locked --bin rover \
    && cp target/release/rover /usr/local/bin/rover

FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 rover \
    && useradd --system --uid 10001 --gid 10001 --create-home --home-dir /home/rover --shell /usr/sbin/nologin rover

COPY --from=builder /usr/local/bin/rover /usr/local/bin/rover

USER rover
WORKDIR /home/rover

ENTRYPOINT ["rover"]
