# syntax=docker/dockerfile:1.7
# Build, run and test image for the Cairn desktop app (DESIGN.md §9.3).
# Stages: base -> dev (interactive work, tests) -> build (release binaries).
# Packaging stages (.deb, install test) are added in Phase 6.

ARG UBUNTU_IMAGE=ubuntu:24.04
FROM ${UBUNTU_IMAGE} AS base
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8

# Build dependencies, runtime libraries, and what the headless test session needs
# (compositors, D-Bus, accessibility bus, software rendering). Package names are
# confirmed for 24.04 and 26.04 in ADR 0001.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates curl git xz-utils build-essential pkg-config mold \
      libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libwebkitgtk-6.0-dev \
      libpoppler-glib-dev libgtk-4-media-gstreamer gstreamer1.0-plugins-good libgtk-4-bin \
      blueprint-compiler gettext desktop-file-utils appstream \
      gvfs gvfs-backends dbus dbus-daemon bubblewrap xdg-user-dirs shared-mime-info \
      mutter weston libgl1-mesa-dri libegl-mesa0 \
      at-spi2-core python3 python3-gi gir1.2-atspi-2.0 python3-pytest python3-pil \
      adwaita-icon-theme yaru-theme-gtk yaru-theme-icon fonts-ubuntu

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    CARGO_TARGET_DIR=/var/cache/cargo-target \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=mold"
WORKDIR /tmp/toolchain
COPY rust-toolchain.toml ./
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none && \
    rustup toolchain install && \
    rustup default "$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)" && \
    rustc --version
WORKDIR /workspace

# ---- dev: interactive development and tests, runs as the host user ----------
FROM base AS dev
# Inherited from base already; repeated so hadolint (which checks each stage on its own) sees it.
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ARG HOST_UID=1000
ARG HOST_GID=1000
ARG NEXTEST_VERSION
ARG NEXTEST_SHA256
ARG WATCHEXEC_VERSION
ARG WATCHEXEC_SHA256
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends gdb sqlite3 wl-clipboard
# Prebuilt, checksum-verified tools (versions in docker/versions.env).
RUN set -eux; \
    curl -sSfL -o /tmp/nextest.tar.gz \
      "https://get.nexte.st/${NEXTEST_VERSION}/x86_64-unknown-linux-gnu.tar.gz"; \
    echo "${NEXTEST_SHA256}  /tmp/nextest.tar.gz" | sha256sum -c -; \
    tar -xzf /tmp/nextest.tar.gz -C /usr/local/cargo/bin cargo-nextest; \
    curl -sSfL -o /tmp/watchexec.tar.xz \
      "https://github.com/watchexec/watchexec/releases/download/v${WATCHEXEC_VERSION}/watchexec-${WATCHEXEC_VERSION}-x86_64-unknown-linux-gnu.tar.xz"; \
    echo "${WATCHEXEC_SHA256}  /tmp/watchexec.tar.xz" | sha256sum -c -; \
    tar -xJf /tmp/watchexec.tar.xz -C /usr/local/bin --strip-components=1 \
      "watchexec-${WATCHEXEC_VERSION}-x86_64-unknown-linux-gnu/watchexec"; \
    rm /tmp/nextest.tar.gz /tmp/watchexec.tar.xz; \
    cargo-nextest --version; watchexec --version
# Ubuntu images ship a default user at UID 1000; replace it with one matching the
# host so files written to the bind-mounted source keep the right owner.
# The cache directories exist (owned by dev) so named volumes inherit that owner.
RUN (userdel -r ubuntu 2>/dev/null || true) && \
    (groupdel ubuntu 2>/dev/null || true) && \
    groupadd -g "${HOST_GID}" dev && \
    useradd -l -m -u "${HOST_UID}" -g "${HOST_GID}" -s /bin/bash dev && \
    mkdir -p "${CARGO_TARGET_DIR}" /usr/local/cargo/registry /usr/local/cargo/git && \
    chown dev:dev /usr/local/cargo "${CARGO_TARGET_DIR}" \
      /usr/local/cargo/registry /usr/local/cargo/git
USER ${HOST_UID}:${HOST_GID}

# ---- build: release binaries -------------------------------------------------
FROM base AS build
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/var/cache/cargo-target \
    cargo build --release --locked -p cairn -p cairn-indexer -p cairn-cli && \
    mkdir -p /out && \
    cp "${CARGO_TARGET_DIR}/release/cairn" "${CARGO_TARGET_DIR}/release/cairn-indexer" \
       "${CARGO_TARGET_DIR}/release/cairn-cli" /out/
