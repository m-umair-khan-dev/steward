# syntax=docker/dockerfile:1.7
# Publishing the apt repository (DESIGN.md §9.5, P6-T09): aptly and gnupg.
# Entry point: docker/scripts/publish-apt.
ARG UBUNTU_IMAGE=ubuntu:24.04
FROM ${UBUNTU_IMAGE}
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8
RUN apt-get update && \
    apt-get install -y --no-install-recommends aptly gnupg ca-certificates dpkg-dev && \
    rm -rf /var/lib/apt/lists/*
COPY docker/scripts/publish-apt docker/scripts/deb-bump /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/publish-apt"]
