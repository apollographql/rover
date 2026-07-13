# syntax=docker/dockerfile:1.25
FROM rust:1.97-trixie AS builder

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

# `--skip-update-check` is baked in: each container has a pinned binary, the
# update-check message recommends the shell installer (wrong surface for
# Docker), and the check pays a per-invocation HTTP round-trip because the
# throttle file in $HOME does not persist across runs.
ENTRYPOINT ["rover", "--skip-update-check"]
