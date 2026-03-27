#!/bin/bash
set -e

VERSION="latest"
INSTALL_DIR="${HOME}/.local/bin"

while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--version)
            VERSION="$2"
            shift 2
            ;;
        -d|--dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -sL "https://api.github.com/repos/dagnele/memento/releases/latest" | grep -o '"tag_name": "[^"]*' | cut -d'"' -f4)
fi

echo "Installing Memento $VERSION to $INSTALL_DIR..."

ASSET_NAME="memento-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
DOWNLOAD_URL="https://github.com/dagnele/memento/releases/download/${VERSION}/${ASSET_NAME}"

mkdir -p "$INSTALL_DIR"

curl -LsSf "$DOWNLOAD_URL" | tar -xz -C "$INSTALL_DIR"

echo "Memento installed successfully!"
echo "Run 'memento --help' to verify."
