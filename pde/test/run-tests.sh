#!/usr/bin/env bash
# Automated tests for pde installer using Docker
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PDE_DIR="$(dirname "$SCRIPT_DIR")"

# Config - single source of truth
UBUNTU_VERSIONS=("22.04" "24.04")
TMUX_VERSION="3.6a"
QUIET="${QUIET:-false}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()  { echo -e "==> $*"; }
pass() { echo -e "${GREEN}PASS${NC}: $*"; }
fail() { echo -e "${RED}FAIL${NC}: $*"; }
warn() { echo -e "${YELLOW}WARN${NC}: $*"; }

has_base_image() {
    docker image inspect "pde-base:$1" &>/dev/null
}

build_base_images() {
    log "Building base images (slow, but only needed once)..."
    for version in "${UBUNTU_VERSIONS[@]}"; do
        log "Building pde-base:$version..."
        if ! docker build -t "pde-base:$version" \
            --build-arg UBUNTU_VERSION="$version" \
            --build-arg TMUX_VERSION="$TMUX_VERSION" \
            -f "$SCRIPT_DIR/Dockerfile.base" \
            "$PDE_DIR"; then
            fail "Failed to build pde-base:$version"
            return 1
        fi
    done
    log "Done. Future tests will be much faster."
}

build_test_image() {
    local version="$1"
    local quiet_flag=""
    [[ "$QUIET" == "true" ]] && quiet_flag="-q"

    if has_base_image "$version"; then
        if ! docker build $quiet_flag -t "pde-test:$version" -f - "$PDE_DIR" <<EOF
FROM pde-base:$version
COPY --chown=testuser:testuser . /home/testuser/.pde
EOF
        then
            fail "Failed to build pde-test:$version"
            return 1
        fi
    else
        warn "No base image for $version - building from scratch (slow)"
        warn "Run '$0 build-base' first for faster tests"
        if ! docker build $quiet_flag -t "pde-test:$version" -f - "$PDE_DIR" <<EOF
FROM ubuntu:$version
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y sudo git curl
RUN useradd -m -s /bin/bash testuser && echo "testuser ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers
COPY --chown=testuser:testuser . /home/testuser/.pde
USER testuser
WORKDIR /home/testuser
EOF
        then
            fail "Failed to build pde-test:$version"
            return 1
        fi
    fi
}

test_profile() {
    local version="$1"
    local profile="$2"
    local logfile="/tmp/pde-test-$version-$profile.log"

    log "Testing Ubuntu $version with profile '$profile'..."
    if ! build_test_image "$version"; then
        return 1
    fi

    if [[ "$QUIET" == "true" ]]; then
        # Quiet mode: capture output, show only on failure
        if docker run --rm "pde-test:$version" bash -c "
            set -e
            cd ~/.pde
            ./pde \"$profile\" 2>&1
            echo '---VERIFY---'
            ./test/verify.sh \"$profile\" \"$TMUX_VERSION\"
        " > "$logfile" 2>&1; then
            # Show just verification output
            sed -n '/^---VERIFY---$/,$ p' "$logfile" | tail -n +2
            pass "Ubuntu $version - $profile"
        else
            # Show full log on failure
            echo "--- Full output ---"
            cat "$logfile"
            fail "Ubuntu $version - $profile"
            return 1
        fi
    else
        # Verbose mode: show everything
        if docker run --rm "pde-test:$version" bash -c "
            set -e
            cd ~/.pde
            ./pde \"$profile\"
            ./test/verify.sh \"$profile\" \"$TMUX_VERSION\"
        "; then
            pass "Ubuntu $version - $profile"
        else
            fail "Ubuntu $version - $profile"
            return 1
        fi
    fi
}

test_idempotency() {
    local version="$1"
    local profile="$2"

    log "Testing idempotency: Ubuntu $version with profile '$profile'..."

    if docker run --rm "pde-test:$version" bash -c "
        set -e
        cd ~/.pde
        ./pde \"$profile\"
        ./pde \"$profile\"
        ./test/verify.sh \"$profile\" \"$TMUX_VERSION\"
    "; then
        pass "Idempotency: Ubuntu $version - $profile"
    else
        fail "Idempotency: Ubuntu $version - $profile"
        return 1
    fi
}

cleanup_images() {
    log "Cleaning up test images..."
    docker images --format '{{.Repository}}:{{.Tag}}' | grep -E '^pde-test:' | while read -r img; do
        docker rmi "$img" 2>/dev/null || true
    done
    log "Done"
}

usage() {
    cat <<EOF
Usage: $0 [command]

Commands:
  minimal      Test minimal profile on all Ubuntu versions
  full         Test full profile on Ubuntu 24.04
  idempotent   Test running installer twice
  all          Run all tests (default)
  build-base   Build base Docker images (run once for faster tests)
  clean        Remove test images

First time:
  $0 build-base && $0 minimal
EOF
}

main() {
    local cmd="${1:-all}"
    local failed=0

    case "$cmd" in
        build-base)
            build_base_images
            ;;
        minimal)
            for v in "${UBUNTU_VERSIONS[@]}"; do
                if ! test_profile "$v" "minimal"; then
                    ((failed++))
                fi
            done
            ;;
        full)
            if ! test_profile "24.04" "full"; then
                ((failed++))
            fi
            ;;
        idempotent)
            if ! test_profile "24.04" "minimal"; then
                ((failed++))
            fi
            if ! test_idempotency "24.04" "minimal"; then
                ((failed++))
            fi
            ;;
        all)
            for v in "${UBUNTU_VERSIONS[@]}"; do
                if ! test_profile "$v" "minimal"; then
                    ((failed++))
                fi
            done
            if ! test_profile "24.04" "full"; then
                ((failed++))
            fi
            if ! test_idempotency "24.04" "minimal"; then
                ((failed++))
            fi
            ;;
        clean)
            cleanup_images
            ;;
        -h|--help|help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown: $cmd"
            usage
            exit 1
            ;;
    esac

    if [[ "$cmd" != "build-base" && "$cmd" != "clean" ]]; then
        echo ""
        if [[ $failed -eq 0 ]]; then
            log "${GREEN}All tests passed!${NC}"
        else
            log "${RED}$failed test(s) failed${NC}"
            exit 1
        fi
    fi
}

main "$@"
