# Every workflow is a make target wrapping Docker Compose (DESIGN.md §9.7).
# Only Docker (with Compose) and make are needed on the host.

SHELL := /bin/bash
.DEFAULT_GOAL := help

include docker/versions.env
-include .env

# Development targets Ubuntu 26.10 for now. 24.04 and 26.04 still build
# (UBUNTU_VERSION=24.04 make check) and join the gate once the project is
# complete (ADR 0007).
UBUNTU_VERSION ?= 26.10
SUPPORTED_RELEASES := 26.10
LATER_RELEASES := 24.04 26.04
HOST_UID := $(shell id -u)
HOST_GID := $(shell id -g)
UBUNTU_IMAGE := $(UBUNTU_IMAGE_$(subst .,_,$(UBUNTU_VERSION)))
ifeq ($(UBUNTU_IMAGE),)
$(error Unsupported UBUNTU_VERSION=$(UBUNTU_VERSION); use one of: $(SUPPORTED_RELEASES) $(LATER_RELEASES))
endif

# Docker Desktop runs containers in a VM that cannot reach the host's Wayland
# socket, so `make app` serves the window over VNC instead (ADR 0004, 0009).
DOCKER_OS := $(shell docker info --format '{{.OperatingSystem}}' 2>/dev/null)
ifneq ($(findstring Docker Desktop,$(DOCKER_OS)),)
APP_SERVICE := app-vnc
else
APP_SERVICE := app
endif

export UBUNTU_VERSION UBUNTU_IMAGE HOST_UID HOST_GID
export NEXTEST_VERSION NEXTEST_SHA256 WATCHEXEC_VERSION WATCHEXEC_SHA256

COMPOSE := docker compose -f compose.desktop.yaml
RUN := $(COMPOSE) run --rm

.PHONY: help init images fixture app shell test e2e lint lint-host check check-all lockfile down clean i18n

help: ## Show this help
	@grep -hE '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-11s %s\n", $$1, $$2}'
	@echo "  Variables: UBUNTU_VERSION=26.10 (default) or 24.04|26.04  k=<test filter for make e2e>"

.env:
	cp .env.example .env

init: .env images fixture ## First-time setup: images and the fake home

images: ## Build the dev image for UBUNTU_VERSION
	$(COMPOSE) --profile build build dev-image

fixture: images ## (Re)create the fake home folder the app runs against
	$(RUN) fixture

app: images ## Run the app against the fake home (restarts on save)
	@if [ "$(APP_SERVICE)" = app-vnc ]; then \
	  echo "Docker Desktop detected: connect a VNC viewer to 127.0.0.1:5900"; fi
	$(RUN) --service-ports $(APP_SERVICE)

shell: images ## A shell with the full toolchain
	$(RUN) shell

lockfile: images ## Create or update Cargo.lock
	$(RUN) shell cargo generate-lockfile

test: images ## Unit and integration tests
	$(RUN) test

e2e: images ## End-to-end tests (make e2e k=E0_03 for one)
	$(RUN) -e E2E_K="$(k)" e2e

i18n: images ## Extract messages to po/cairn.pot, merge into po/*.po, remake the pseudo-locale
	$(RUN) shell bash -c 'docker/scripts/i18n extract && docker/scripts/i18n update && docker/scripts/i18n pseudo && docker/scripts/i18n check'

lint: images lint-host ## Formatting, clippy, Dockerfile and Compose checks
	$(RUN) lint

lint-host:
	docker run --rm -i $(HADOLINT_IMAGE) hadolint --ignore DL3008 --ignore DL3059 - < docker/desktop.Dockerfile
	$(COMPOSE) config -q
	$(COMPOSE) --profile build config -q

check: lint test e2e ## The gate on UBUNTU_VERSION
	@echo "check: all green on Ubuntu $(UBUNTU_VERSION)"

check-all: ## The gate on every supported Ubuntu release
	@set -e; for v in $(SUPPORTED_RELEASES); do $(MAKE) --no-print-directory check UBUNTU_VERSION=$$v; done
	@echo "check-all: all green on $(SUPPORTED_RELEASES)"

down: ## Stop containers
	$(COMPOSE) down --remove-orphans

clean: ## Stop containers and delete build caches and the fake home
	$(COMPOSE) down --remove-orphans --volumes
	-docker volume rm cairn-target-24.04 cairn-target-26.04 cairn-target-26.10
