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

build_image() {
	local version="$1" base
	base="$(base_image "$version")"
	if ! docker image inspect "$base" >/dev/null 2>&1; then
		docker build -t "$base" --build-arg "UBUNTU_VERSION=$version" -f "$SCRIPT_DIR/Dockerfile.base" "$REPO_ROOT"
	fi
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
	if ! docker image inspect "$base" >/dev/null 2>&1; then
		docker build -t "$base" --build-arg UBUNTU_VERSION=22.04 -f "$SCRIPT_DIR/Dockerfile.terminal" "$REPO_ROOT"
	fi
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
