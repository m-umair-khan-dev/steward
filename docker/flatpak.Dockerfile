# Flatpak builder (P9-T02; ADR 0015). flatpak-builder sandboxes each build
# step with bwrap, so the container runs with the same relaxed seccomp and
# AppArmor as the dev container (compose.desktop.yaml). Use `make flatpak`.
ARG UBUNTU_IMAGE=ubuntu:26.10
FROM ${UBUNTU_IMAGE}
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      flatpak flatpak-builder ca-certificates git xz-utils elfutils \
      dbus dbus-x11 && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
