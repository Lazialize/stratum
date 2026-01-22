#!/bin/bash
# Build script for creating optimized release binaries
# This script builds Stratum for multiple platforms and creates distribution archives

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get version from Cargo.toml
VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d '"' -f 2)
echo -e "${GREEN}Building Stratum v${VERSION}${NC}"

# Create dist directory
DIST_DIR="dist"
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

# Function to build for a target
build_target() {
    local target=$1
    local name=$2

    echo -e "${YELLOW}Building for ${name} (${target})...${NC}"

    # Check if target is installed
    if ! rustup target list --installed | grep -q "^${target}$"; then
        echo -e "${YELLOW}Installing target ${target}...${NC}"
        rustup target add "${target}"
    fi

    # Build
    cargo build --release --target "${target}"

    echo -e "${GREEN}✓ Built ${name}${NC}"
}

# Function to create archive
create_archive() {
    local target=$1
    local name=$2
    local binary_name="stratum"
    local archive_name="stratum-${VERSION}-${name}"

    echo -e "${YELLOW}Creating archive for ${name}...${NC}"

    # Windows uses .exe extension
    if [[ "${target}" == *"windows"* ]]; then
        binary_name="stratum.exe"
        archive_name="${archive_name}.zip"

        # Create zip archive
        cd "target/${target}/release"
        zip "../../../${DIST_DIR}/${archive_name}" "${binary_name}"
        cd ../../..
    else
        archive_name="${archive_name}.tar.gz"

        # Create tar.gz archive
        tar -czf "${DIST_DIR}/${archive_name}" \
            -C "target/${target}/release" \
            "${binary_name}"
    fi

    # Generate checksum
    shasum -a 256 "${DIST_DIR}/${archive_name}" > "${DIST_DIR}/${archive_name}.sha256"

    echo -e "${GREEN}✓ Created ${archive_name}${NC}"
}

# Build for current platform (default)
echo -e "${GREEN}Building for current platform...${NC}"
cargo build --release

# Detect current platform
CURRENT_ARCH=$(uname -m)
CURRENT_OS=$(uname -s)

if [[ "$CURRENT_OS" == "Darwin" ]]; then
    # macOS builds
    echo -e "${GREEN}=== Building macOS binaries ===${NC}"

    if [[ "$CURRENT_ARCH" == "arm64" ]]; then
        # On Apple Silicon, build both architectures
        build_target "aarch64-apple-darwin" "macos-arm64"
        build_target "x86_64-apple-darwin" "macos-x86_64"

        # Create universal binary
        echo -e "${YELLOW}Creating universal binary...${NC}"
        lipo -create \
            target/aarch64-apple-darwin/release/stratum \
            target/x86_64-apple-darwin/release/stratum \
            -output "${DIST_DIR}/stratum-universal"

        # Create universal archive
        tar -czf "${DIST_DIR}/stratum-${VERSION}-macos-universal.tar.gz" \
            -C "${DIST_DIR}" \
            stratum-universal
        shasum -a 256 "${DIST_DIR}/stratum-${VERSION}-macos-universal.tar.gz" \
            > "${DIST_DIR}/stratum-${VERSION}-macos-universal.tar.gz.sha256"

        echo -e "${GREEN}✓ Created universal binary${NC}"
    else
        # On Intel Mac
        build_target "x86_64-apple-darwin" "macos-x86_64"
        create_archive "x86_64-apple-darwin" "macos-x86_64"
    fi
elif [[ "$CURRENT_OS" == "Linux" ]]; then
    # Linux builds
    echo -e "${GREEN}=== Building Linux binaries ===${NC}"

    if [[ "$CURRENT_ARCH" == "x86_64" ]]; then
        build_target "x86_64-unknown-linux-gnu" "linux-x86_64"
        create_archive "x86_64-unknown-linux-gnu" "linux-x86_64"

        # Also build musl version for portability
        if command -v x86_64-linux-musl-gcc &> /dev/null; then
            build_target "x86_64-unknown-linux-musl" "linux-x86_64-musl"
            create_archive "x86_64-unknown-linux-musl" "linux-x86_64-musl"
        else
            echo -e "${YELLOW}Skipping musl build (musl-gcc not found)${NC}"
        fi
    fi
fi

# Optional: Cross-compile for other platforms (requires toolchains)
if [[ "${BUILD_ALL_PLATFORMS}" == "true" ]]; then
    echo -e "${GREEN}=== Cross-compiling for all platforms ===${NC}"

    # Linux
    if command -v x86_64-linux-gnu-gcc &> /dev/null; then
        build_target "x86_64-unknown-linux-gnu" "linux-x86_64"
        create_archive "x86_64-unknown-linux-gnu" "linux-x86_64"
    fi

    # Windows
    if command -v x86_64-w64-mingw32-gcc &> /dev/null; then
        build_target "x86_64-pc-windows-gnu" "windows-x86_64"
        create_archive "x86_64-pc-windows-gnu" "windows-x86_64"
    fi
fi

# Summary
echo -e "${GREEN}=== Build Summary ===${NC}"
echo "Version: ${VERSION}"
echo "Distribution directory: ${DIST_DIR}/"
echo ""
echo "Built artifacts:"
ls -lh "${DIST_DIR}/"

echo -e "${GREEN}✓ Build complete!${NC}"
