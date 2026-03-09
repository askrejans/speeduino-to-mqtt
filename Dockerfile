# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

# Build dependencies: perl + cmake for openssl-src, pkg-config for linkage
RUN apt-get update && apt-get install -y --no-install-recommends \
        perl \
        make \
        cmake \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies: copy manifests first, build a dummy main, then replace
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release --locked 2>/dev/null || true \
    && rm -rf src

# Build the real binary
COPY . .
RUN cargo build --release --locked

# ── Runtime stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r speeduino \
    && useradd -r -g speeduino \
        --groups dialout,tty \
        --no-create-home \
        --shell /usr/sbin/nologin \
        speeduino \
    && mkdir -p /etc/speeduino-to-mqtt

COPY --from=builder /app/target/release/speeduino-to-mqtt /usr/local/bin/speeduino-to-mqtt
COPY example.settings.toml /etc/speeduino-to-mqtt/settings.toml

USER speeduino
WORKDIR /tmp

# Force service/log mode – no TUI in a container.
# Override with SPEEDUINO_NO_TUI=0 if you run with `docker run -it`.
ENV SPEEDUINO_NO_TUI=1

# All configuration is driven by environment variables (SPEEDUINO_* prefix)
# or by mounting a settings.toml over /etc/speeduino-to-mqtt/settings.toml.
CMD ["speeduino-to-mqtt", "--config", "/etc/speeduino-to-mqtt/settings.toml"]
