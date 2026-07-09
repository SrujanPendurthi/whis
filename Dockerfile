# syntax=docker/dockerfile:1
#
# whis — reproducible build/CI/dev toolchain.
#
# SCOPE: This image bakes in every native dependency the Rust workspace and the
# Vite frontend need (Rust 1.96, Node 24, protoc, and the full WebKitGTK/Tauri
# system-dep stack). Use it to run `cargo test`/`clippy`/`fmt`, build the
# frontend, or produce a *Linux* Tauri bundle — all without touching a host
# toolchain.
#
# NOT IN SCOPE: the shipping macOS .app/.dmg and the live GUI. Those require
# macOS + Xcode on the host (Metal/candle `metal` feature, .icns bundling, code
# signing). Build those natively with `npm run tauri build`. Docker here is for
# dependency reproducibility and CI, not for producing the mac desktop artifact.
#
# Pinned to match the host toolchain (rustc 1.96.0, node 24).

FROM rust:1.96-bookworm

# --- System dependencies -----------------------------------------------------
# protobuf-compiler: lancedb's build script requires `protoc`.
# libwebkit2gtk-4.1-dev + libayatana-appindicator3-dev + librsvg2-dev + libxdo-dev:
#   the Tauri v2 Linux runtime/link deps.
# clang/libclang: bindgen-based -sys crates.
# cmake/pkg-config/libssl-dev: common native build + reqwest(openssl) linkage.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        file \
        libclang-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libssl-dev \
        libwebkit2gtk-4.1-dev \
        libxdo-dev \
        pkg-config \
        protobuf-compiler \
        wget \
    && rm -rf /var/lib/apt/lists/*

# --- Node.js 24 (NodeSource) --------------------------------------------------
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && node --version && npm --version

# Rust components used by CI (fmt/clippy).
RUN rustup component add rustfmt clippy

WORKDIR /app

# Default to an interactive shell; compose/CI override with an explicit command.
CMD ["bash"]
