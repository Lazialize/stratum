#!/bin/bash
# Cross-platform build script using 'cross' tool
# This script builds Strata for multiple platforms from any host OS

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get version from Cargo.toml
VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d '"' -f 2)
echo -e "${GREEN}Building Strata v${VERSION} for all platforms${NC}"

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo -e "${YELLOW}Installing 'cross' for cross-compilation...${NC}"
    cargo install cross --git https://github.com/cross-rs/cross
fi

# Check if Docker is running (required by cross)
if ! docker info &> /dev/null; then
    echo -e "${RED}Error: Docker is not running. Please start Docker and try again.${NC}"
    echo -e "${YELLOW}Cross-compilation requires Docker to be running.${NC}"
    exit 1
fi

# Create dist directory
DIST_DIR="dist"
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

# Function to build for a target using cross
build_target() {
    local target=$1
    local name=$2

    echo -e "${YELLOW}Building for ${name} (${target})...${NC}"

    # Build using cross (cross manages targets internally via Docker)
    # -p strata is required for workspace projects
    cross build -p strata --release --target "${target}"

    echo -e "${GREEN}✓ Built ${name}${NC}"
}

# Function to create archive
create_archive() {
    local target=$1
    local name=$2
    local binary_name="strata"
    local archive_name="strata-${VERSION}-${name}.tar.gz"

    echo -e "${YELLOW}Creating archive for ${name}...${NC}"

    # Create tar.gz archive
    tar -czf "${DIST_DIR}/${archive_name}" \
        -C "target/${target}/release" \
        "${binary_name}"

    # Generate checksum
    shasum -a 256 "${DIST_DIR}/${archive_name}" > "${DIST_DIR}/${archive_name}.sha256"

    echo -e "${GREEN}✓ Created ${archive_name}${NC}"
}

echo -e "${GREEN}=== Building for all platforms ===${NC}"

# macOS builds (can only be built natively on macOS)
CURRENT_OS=$(uname -s)
if [[ "$CURRENT_OS" == "Darwin" ]]; then
    echo -e "${GREEN}Building macOS binaries natively...${NC}"

    # macOS x86_64
    echo -e "${YELLOW}Building for macOS x86_64...${NC}"
    rustup target add x86_64-apple-darwin
    cargo build -p strata --release --target x86_64-apple-darwin
    create_archive "x86_64-apple-darwin" "macos-x86_64"

    # macOS ARM64
    echo -e "${YELLOW}Building for macOS ARM64...${NC}"
    rustup target add aarch64-apple-darwin
    cargo build -p strata --release --target aarch64-apple-darwin
    create_archive "aarch64-apple-darwin" "macos-arm64"

    # Create universal binary
    echo -e "${YELLOW}Creating universal binary...${NC}"
    lipo -create \
        target/aarch64-apple-darwin/release/strata \
        target/x86_64-apple-darwin/release/strata \
        -output "${DIST_DIR}/strata-universal"

    tar -czf "${DIST_DIR}/strata-${VERSION}-macos-universal.tar.gz" \
        -C "${DIST_DIR}" \
        strata-universal
    shasum -a 256 "${DIST_DIR}/strata-${VERSION}-macos-universal.tar.gz" \
        > "${DIST_DIR}/strata-${VERSION}-macos-universal.tar.gz.sha256"

    echo -e "${GREEN}✓ Created universal binary${NC}"
else
    echo -e "${YELLOW}Skipping macOS builds (can only be built on macOS)${NC}"
fi

# Linux builds using cross
echo -e "${GREEN}Building Linux binaries using cross...${NC}"

build_target "x86_64-unknown-linux-gnu" "linux-x86_64"
create_archive "x86_64-unknown-linux-gnu" "linux-x86_64"

build_target "aarch64-unknown-linux-gnu" "linux-arm64"
create_archive "aarch64-unknown-linux-gnu" "linux-arm64"

# Summary
echo -e "${GREEN}=== Build Summary ===${NC}"
echo "Version: ${VERSION}"
echo "Distribution directory: ${DIST_DIR}/"
echo ""
echo "Built artifacts:"
ls -lh "${DIST_DIR}/"

echo -e "${GREEN}✓ Cross-platform build complete!${NC}"
echo ""
echo -e "${YELLOW}Note: macOS binaries can only be built on macOS hosts${NC}"
