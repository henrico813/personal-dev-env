#!/usr/bin/env bash
# Bootstrap PDE installation from curl
set -euo pipefail

REPO_URL="https://git.blathers.goog/hco/pde.git"
INSTALL_DIR="$HOME/.pde"

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
        git -C "$INSTALL_DIR" pull
    else
        echo "Cloning PDE..."
        git clone "$REPO_URL" "$INSTALL_DIR"
    fi

    # Run installer
    "$INSTALL_DIR/pde" "$profile"
}

main "$@"
