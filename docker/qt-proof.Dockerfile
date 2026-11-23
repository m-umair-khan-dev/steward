# syntax=docker/dockerfile:1.7
# Qt 6 / QML proof (spikes/qt-proof): the 26.10 dev image plus Qt's
# development packages. Throwaway; if Qt is adopted, these packages move into
# docker/desktop.Dockerfile.
ARG BASE=steward-dev:26.10
FROM ${BASE}
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
USER root
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && \
    apt-get install -y --no-install-recommends \
      qt6-base-dev qt6-base-dev-tools qt6-declarative-dev qt6-declarative-dev-tools qmake6 \
      qml6-module-qtquick qml6-module-qtquick-window qml6-module-qtquick-effects \
      qml6-module-qtquick-shapes qml6-module-qtqml-workerscript qml6-module-qtquick-templates \
      qml6-module-qtquick-controls qml6-module-qtquick-layouts \
      qt6-wayland qt6-qpa-plugins libgl-dev
ENV QMAKE=qmake6 \
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
USER 1000:1000
