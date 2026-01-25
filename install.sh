#!/bin/bash
set -e

# Strata Installation Script
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Lazialize/strata/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/Lazialize/strata/main/install.sh | VERSION=v1.0.0 bash

# Configuration
REPO="Lazialize/strata"
BINARY_NAME="strata"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Version specification (can be overridden with environment variable, defaults to latest)
VERSION="${VERSION:-latest}"

# Colored output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        *)          error "Unsupported OS: $(uname -s)";;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64";;
        aarch64|arm64)  echo "arm64";;
        *)              error "Unsupported architecture: $(uname -m)";;
    esac
}

# Check required commands
check_dependencies() {
    for cmd in curl tar; do
        if ! command -v "$cmd" &> /dev/null; then
            error "Required command not found: $cmd"
        fi
    done
}

# Get latest version
get_latest_version() {
    info "Fetching latest version..." >&2
    local latest

    # Get the latest stable release (excludes pre-releases)
    latest=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$latest" ]; then
        error "No stable release found. Please specify a version using VERSION=vX.Y.Z environment variable (e.g., VERSION=v0.1.0-nightly-20260125.1341)."
    fi

    echo "$latest"
}

# Verify checksum
verify_checksum() {
    local file="$1"
    local checksum_url="$2"

    if command -v sha256sum &> /dev/null; then
        info "Verifying checksum..."
        local expected_checksum
        expected_checksum=$(curl -fsSL "$checksum_url" | awk '{print $1}')

        if [ -z "$expected_checksum" ]; then
            warn "Could not fetch checksum, skipping verification"
            return 0
        fi

        local actual_checksum
        actual_checksum=$(sha256sum "$file" | awk '{print $1}')

        if [ "$expected_checksum" != "$actual_checksum" ]; then
            error "Checksum verification failed! Expected: $expected_checksum, Got: $actual_checksum"
        fi

        info "Checksum verified successfully"
    elif command -v shasum &> /dev/null; then
        info "Verifying checksum..."
        local expected_checksum
        expected_checksum=$(curl -fsSL "$checksum_url" | awk '{print $1}')

        if [ -z "$expected_checksum" ]; then
            warn "Could not fetch checksum, skipping verification"
            return 0
        fi

        local actual_checksum
        actual_checksum=$(shasum -a 256 "$file" | awk '{print $1}')

        if [ "$expected_checksum" != "$actual_checksum" ]; then
            error "Checksum verification failed! Expected: $expected_checksum, Got: $actual_checksum"
        fi

        info "Checksum verified successfully"
    else
        warn "sha256sum/shasum not found, skipping checksum verification"
    fi
}

# Download and install binary
install_binary() {
    local os="$1"
    local arch="$2"
    local version="$3"

    # Get latest version if not specified
    if [ "$version" = "latest" ]; then
        version=$(get_latest_version)
    fi

    info "Version: $version"
    info "OS: $os"
    info "Architecture: $arch"

    # Build download URL
    # Format: strata--{os}-{arch}.tar.gz
    local archive_name="${BINARY_NAME}--${os}-${arch}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"
    local checksum_url="${download_url}.sha256"

    info "Downloading: $archive_name"

    # Create temporary directory
    local tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    # Download archive
    if ! curl -fsSL "$download_url" -o "$tmp_dir/$archive_name" 2>/dev/null; then
        error "Failed to download binary. The specified version ($version) and architecture ($os-$arch) combination may not exist."
    fi

    # Verify checksum
    verify_checksum "$tmp_dir/$archive_name" "$checksum_url"

    info "Extracting..."
    tar -xzf "$tmp_dir/$archive_name" -C "$tmp_dir"

    # Find binary file (should be directly in the archive root)
    local binary_path="$tmp_dir/$BINARY_NAME"
    if [ ! -f "$binary_path" ]; then
        error "Binary file not found in archive"
    fi

    # Create installation directory
    mkdir -p "$INSTALL_DIR"

    info "Installing: $INSTALL_DIR/$BINARY_NAME"
    cp "$binary_path" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    info "Installation completed!"
}

# Check PATH and warn if needed
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "Installation directory ($INSTALL_DIR) is not in your PATH."
        warn "Add the following command to your shell configuration file (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
    fi
}

# Verify installation
verify_installation() {
    if [ -x "$INSTALL_DIR/$BINARY_NAME" ]; then
        info "Verifying installation:"
        "$INSTALL_DIR/$BINARY_NAME" --version || true
        echo ""
        info "For usage information, run:"
        echo "  $BINARY_NAME --help"
    fi
}

# Main process
main() {
    info "Strata Installation Script"
    echo ""

    # Check dependencies
    check_dependencies

    # Detect system information
    local os=$(detect_os)
    local arch=$(detect_arch)

    # Execute installation
    install_binary "$os" "$arch" "$VERSION"

    # Check PATH
    check_path

    # Verify installation
    verify_installation

    echo ""
    info "Installation successful!"
}

# Execute script
main
