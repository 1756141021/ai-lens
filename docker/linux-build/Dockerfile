# Linux build + X11 smoke environment for AI Lens.
# Mirrors GitHub Actions ubuntu-24.04 (same webkitgtk 4.1 line); CN mirrors for
# apt/rustup/crates/npm because this image builds on a machine behind the GFW.
#
# Build:  docker build -t ailens-build docker/linux-build
# Use:    docker run --rm -v <repo>:/work \
#           -v ailens-cargo:/root/.cargo/registry \
#           -v ailens-target:/work/src-tauri/target \
#           -v ailens-node:/work/node_modules \
#           -v ailens-cache:/root/.cache \
#           ailens-build bash -c "pnpm install && pnpm tauri build"
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
# http on purpose: the base image ships no ca-certificates, and apt verifies
# packages by GPG signature regardless of transport.
RUN sed -i 's|http://archive.ubuntu.com|http://mirrors.tuna.tsinghua.edu.cn|; s|http://security.ubuntu.com|http://mirrors.tuna.tsinghua.edu.cn|' /etc/apt/sources.list.d/ubuntu.sources \
    && apt-get update && apt-get install -y --no-install-recommends \
    # Tauri v2 prerequisites (per https://v2.tauri.app/start/prerequisites/)
    libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
    pkg-config ca-certificates git xz-utils \
    # xcap's Linux backend (keep in sync with the CI workflow)
    libpipewire-0.3-dev clang \
    # AppImage bundling inside a container (no FUSE; noble renamed the package)
    libfuse2t64 desktop-file-utils \
    # Xvfb smoke: virtual display + window probing + frame dumps
    xvfb x11-utils x11-apps imagemagick dbus dbus-x11 \
    && rm -rf /var/lib/apt/lists/*

# Rust via rsproxy (ByteDance mirror of static.rust-lang.org + crates.io)
ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
COPY cargo-config.toml /root/.cargo/config.toml

# Node 22 (npmmirror binary dist) + pnpm
RUN curl -fsSL https://npmmirror.com/mirrors/node/v22.14.0/node-v22.14.0-linux-x64.tar.xz -o /tmp/node.tar.xz \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 && rm /tmp/node.tar.xz \
    && npm config set registry https://registry.npmmirror.com \
    && npm install -g pnpm \
    && pnpm config set registry https://registry.npmmirror.com \
    # keep the store off /work — pnpm would otherwise drop .pnpm-store into the
    # bind-mounted repo (cross-filesystem fallback) and flood source control
    && pnpm config set store-dir /root/.pnpm-store

# linuxdeploy/AppImage tools must self-extract instead of FUSE-mounting
ENV APPIMAGE_EXTRACT_AND_RUN=1 \
    NO_STRIP=1

WORKDIR /work
