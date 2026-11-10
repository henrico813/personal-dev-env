#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
UBUNTU_VERSIONS=("22.04" "24.04")

base_image() {
	local version="$1" identity
	identity="$(sha256sum "$SCRIPT_DIR/Dockerfile.base")"
	printf 'pde-base:%s-%s\n' "$version" "${identity%% *}"
}

terminal_base_image() {
	local identity
	identity="$(sha256sum "$SCRIPT_DIR/Dockerfile.terminal")"
	printf 'pde-terminal-base:22.04-%s\n' "${identity%% *}"
}

# Builds a base image unless it already exists. When PDE_IMAGE_CACHE names a
# folder, the image is loaded from or saved to it, so CI runs apt-get only
# after a Dockerfile changes. PDE_UBUNTU_MIRROR replaces archive.ubuntu.com.
# CI sets it to Azure's mirror, which refuses package downloads from outside
# Azure.
ensure_base_image() {
	local image="$1" dockerfile="$2" version="$3" archive=""
	if docker image inspect "$image" >/dev/null 2>&1; then
		return
	fi
	if [[ -n "${PDE_IMAGE_CACHE:-}" ]]; then
		archive="$PDE_IMAGE_CACHE/${image//:/-}.tar"
		if [[ -f "$archive" ]]; then
			docker load -i "$archive"
			return
		fi
	fi
	docker build -t "$image" \
		--build-arg "UBUNTU_VERSION=$version" \
		--build-arg "UBUNTU_MIRROR=${PDE_UBUNTU_MIRROR:-}" \
		-f "$SCRIPT_DIR/$dockerfile" "$REPO_ROOT"
	if [[ -n "$archive" ]]; then
		mkdir -p "$PDE_IMAGE_CACHE"
		docker save -o "$archive" "$image"
	fi
}

build_image() {
	local version="$1" base
	base="$(base_image "$version")"
	ensure_base_image "$base" Dockerfile.base "$version"
	docker build -t "pde-smoke:$version" --build-arg "BASE_IMAGE=$base" -f "$SCRIPT_DIR/Dockerfile" "$REPO_ROOT"
}

smoke() {
	local version
	for version in "${UBUNTU_VERSIONS[@]}"; do
		build_image "$version"
		docker run --rm --user root "pde-smoke:$version" sh -c 'if pde-installer doctor --repo-root /home/testuser/personal-dev-env; then exit 1; fi'
		docker run --rm "pde-smoke:$version" ./pde-installer/test/verify.sh
	done
}

terminal() {
	local base
	base="$(terminal_base_image)"
	ensure_base_image "$base" Dockerfile.terminal 22.04
	docker build -t pde-installer-terminal --build-arg "BASE_IMAGE=$base" -f "$SCRIPT_DIR/Dockerfile" "$REPO_ROOT"
	docker run --rm pde-installer-terminal ./pde-installer/test/verify-terminal.sh
}

clean() {
	local version
	for version in "${UBUNTU_VERSIONS[@]}"; do
		docker image rm "pde-smoke:$version" "$(base_image "$version")" 2>/dev/null || true
	done
}

case "${1:-smoke}" in
	smoke) smoke ;;
	terminal) terminal ;;
	clean) clean ;;
	*) printf 'usage: %s [smoke|terminal|clean]\n' "$0" >&2; exit 2 ;;
esac
