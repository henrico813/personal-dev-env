#!/usr/bin/env bash
# Bootstrap PDE installation from curl
set -euo pipefail

REPO_URL="https://github.com/henrico813/personal-dev-env.git"
INSTALL_DIR="$HOME/.pde"
DEFAULT_BRANCH="main"

update_install() {
    if ! git -C "$INSTALL_DIR" rev-parse --is-inside-work-tree &>/dev/null; then
        echo "Error: $INSTALL_DIR exists but is not a git repository"
        echo "Remove it manually or move it aside, then retry."
        exit 1
    fi

    if git -C "$INSTALL_DIR" remote get-url origin &>/dev/null; then
        git -C "$INSTALL_DIR" remote set-url origin "$REPO_URL"
    else
        git -C "$INSTALL_DIR" remote add origin "$REPO_URL"
    fi

    git -C "$INSTALL_DIR" fetch origin "$DEFAULT_BRANCH"
    git -C "$INSTALL_DIR" checkout "$DEFAULT_BRANCH"
    git -C "$INSTALL_DIR" pull --ff-only origin "$DEFAULT_BRANCH"
}

run_installer() {
    local profile="$1"

    if [[ -x "$INSTALL_DIR/pde/pde" ]]; then
        "$INSTALL_DIR/pde/pde" "$profile"
        return 0
    fi

    if [[ -x "$INSTALL_DIR/pde" ]]; then
        "$INSTALL_DIR/pde" "$profile"
        return 0
    fi

    echo "Error: PDE installer not found after clone/update"
    exit 1
}

main() {
    local profile="${1:-}"

    if [[ -z "$profile" ]]; then
        echo "Usage: curl ... | bash -s -- <minimal|full>"
        exit 1
    fi

    # Install git if missing
    if ! command -v git &>/dev/null; then
        echo "Installing git..."
        sudo apt-get update -qq
        sudo apt-get install -y git
    fi

    # Clone or update
    if [[ -d "$INSTALL_DIR" ]]; then
        echo "Updating existing PDE installation..."
        update_install
    else
        echo "Cloning PDE..."
        git clone --branch "$DEFAULT_BRANCH" --single-branch "$REPO_URL" "$INSTALL_DIR"
    fi

    # Run installer
    run_installer "$profile"
}

main "$@"
