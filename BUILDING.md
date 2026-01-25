# Building Strata

This document describes how to build Strata from source for different platforms.

## Prerequisites

- Rust 1.92 or later
- Cargo (comes with Rust)
- Platform-specific toolchains for cross-compilation (see below)

## Quick Start

### Workspace Structure

This repository uses a virtual workspace structure. Crates are organized as `src/cli` (CLI), `src/core` (domain logic), and `src/db` (database adapters). The build target is the `strata` crate.

### Development Build

For development and testing:

```bash
cargo build -p strata
```

Binary location: `target/debug/strata`

### Release Build

Optimized production build:

```bash
cargo build -p strata --release
```

Binary location: `target/release/strata`

## Release Profile Optimizations

The release profile in `Cargo.toml` includes the following optimizations:

- **opt-level = 3**: Maximum optimization level
- **lto = true**: Link-Time Optimization for better performance and smaller binary size
- **codegen-units = 1**: Single codegen unit for better optimization (slower compile, faster runtime)
- **strip = true**: Remove debug symbols to reduce binary size
- **panic = "abort"**: Abort on panic instead of unwinding (smaller binary, faster panic)

These settings prioritize:
1. Runtime performance
2. Binary size reduction
3. Production readiness

## Cross-Compilation

Strata supports building for multiple platforms from a single development machine.

### Automated Build Script (Recommended)

The project includes an automated build script for multiple platforms:

```bash
./scripts/cross-build.sh
```

This script automatically:
- Installs the `cross` tool (if not already installed)
- Builds for Linux x86_64 and ARM64 (cross-compilation via Docker containers)
- Builds for macOS Intel/Apple Silicon/Universal binaries (macOS only)
- Generates tar.gz archives and SHA256 checksums
- Outputs all artifacts to the `dist/` directory

**Requirements:**
- Docker Desktop (macOS/Windows) or Docker Engine (Linux) must be running
- macOS environment required for building macOS binaries

**Generated files:**
```
dist/
├── strata-{VERSION}-linux-x86_64.tar.gz
├── strata-{VERSION}-linux-x86_64.tar.gz.sha256
├── strata-{VERSION}-linux-arm64.tar.gz
├── strata-{VERSION}-linux-arm64.tar.gz.sha256
├── strata-{VERSION}-macos-x86_64.tar.gz
├── strata-{VERSION}-macos-x86_64.tar.gz.sha256
├── strata-{VERSION}-macos-arm64.tar.gz
├── strata-{VERSION}-macos-arm64.tar.gz.sha256
├── strata-{VERSION}-macos-universal.tar.gz
└── strata-{VERSION}-macos-universal.tar.gz.sha256
```

### Manual Cross-Compilation with `cross`

You can also build for individual targets using `cross`.

#### Installation

```bash
cargo install cross --git https://github.com/cross-rs/cross
```

#### Usage

Since this project uses a workspace structure, you must specify the `-p strata` flag:

```bash
# Build for Linux ARM64 (from macOS/Windows/Linux)
cross build -p strata --release --target aarch64-unknown-linux-gnu

# Build for Linux x86_64
cross build -p strata --release --target x86_64-unknown-linux-gnu
```

Binary location: `target/{target}/release/strata`

**Requirements:**
- Docker Desktop (macOS/Windows) or Docker Engine (Linux) must be running
- `Cross.toml` configuration file in project root (already provided)

**Supported targets:**
- `x86_64-unknown-linux-gnu` - Linux x86_64
- `aarch64-unknown-linux-gnu` - Linux ARM64

**Limitations:**
- macOS targets (`*-apple-darwin`) can only be built on macOS

### Native macOS Builds

Building for macOS requires a macOS environment:

```bash
# For Intel Macs
rustup target add x86_64-apple-darwin
cargo build -p strata --release --target x86_64-apple-darwin

# For Apple Silicon (M1/M2/M3)
rustup target add aarch64-apple-darwin
cargo build -p strata --release --target aarch64-apple-darwin

# Universal Binary (supports both)
lipo -create \
  target/x86_64-apple-darwin/release/strata \
  target/aarch64-apple-darwin/release/strata \
  -output strata-universal
```

**Requirements:**
- macOS environment
- Xcode Command Line Tools: `xcode-select --install`

---

## Advanced: Native Toolchain Cross-Compilation

For advanced users: Cross-compiling without `cross` using native toolchains.

Using native toolchains requires installing platform-specific toolchains and linkers.

#### Installing Targets

```bash
# Linux targets
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu

# macOS targets (macOS only)
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

#### Linux Cross-Compilation Toolchains

Building for Linux targets from macOS:

```bash
# Install via Homebrew
brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
```

Building for other architectures on Linux:

```bash
# Ubuntu/Debian
sudo apt-get install gcc-x86-64-linux-gnu
sudo apt-get install gcc-aarch64-linux-gnu
```

#### Build Examples

```bash
# Linux x86_64
cargo build -p strata --release --target x86_64-unknown-linux-gnu

# Linux ARM64
cargo build -p strata --release --target aarch64-unknown-linux-gnu
```

Binary location: `target/{target}/release/strata`

**Note:** Cross-compiling with native toolchains is complex. **Using the `cross` tool is strongly recommended.**

## Binary Size Optimization

The release profile already includes aggressive size optimizations. Additional techniques:

### 1. Strip Debug Symbols (if not using strip = true)

```bash
strip target/release/strata
```

### 2. Use UPX Compression (Optional)

```bash
# Install UPX
brew install upx  # macOS
apt-get install upx  # Linux

# Compress binary
upx --best --lzma target/release/strata
```

**Note**: UPX-compressed binaries may trigger antivirus false positives.

### 3. Optimize Dependencies

Add to `Cargo.toml`:

```toml
[profile.release]
# Already configured with optimal settings
```

---

## Troubleshooting

### Docker Not Running

If you encounter Docker errors when using `cross`:

```bash
# Check Docker status
docker info

# macOS: Start Docker Desktop
# Linux: Start Docker service
sudo systemctl start docker
```

### Build Failures

If builds fail:

1. Clear cache and retry:
```bash
cargo clean
./scripts/cross-build.sh
```

2. Update `cross` to the latest version:
```bash
cargo install cross --git https://github.com/cross-rs/cross --force
```

3. Pull Docker images again:
```bash
docker pull ghcr.io/cross-rs/x86_64-unknown-linux-gnu:latest
docker pull ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest
```

### Large Binary Size

If binary size is larger than expected:

1. Verify `strip = true` is set: [Cargo.toml](Cargo.toml#L10)
2. Check for debug symbols:
```bash
file target/release/strata
```

3. Analyze size with `cargo bloat`:
```bash
cargo install cargo-bloat
cargo bloat -p strata --release
```

---

## CI/CD Integration

Example cross-platform build workflow for GitHub Actions:

```yaml
name: Release Build
on:
  push:
    tags:
      - 'v*.*.*'

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Install cross
        if: matrix.os == 'ubuntu-latest'
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build
        run: |
          if [ "${{ matrix.os }}" = "ubuntu-latest" ]; then
            cross build -p strata --release --target ${{ matrix.target }}
          else
            cargo build -p strata --release --target ${{ matrix.target }}
          fi

      - name: Create archive
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../strata-${{ matrix.target }}.tar.gz strata

      - uses: actions/upload-artifact@v4
        with:
          name: strata-${{ matrix.target }}
          path: strata-${{ matrix.target }}.tar.gz
```

See [.github/workflows/](.github/workflows/) for more details.

---

## Testing and Distribution

### Testing Release Builds

Always test release builds before distribution:

```bash
# Build release
cargo build -p strata --release

# Run tests in release mode
cargo test -p strata --release
```

### Manual Distribution

To create distribution artifacts manually without `./scripts/cross-build.sh`:

```bash
# Create archive
tar -czf strata-linux-x86_64.tar.gz -C target/x86_64-unknown-linux-gnu/release strata

# Generate checksum
shasum -a 256 strata-linux-x86_64.tar.gz > strata-linux-x86_64.tar.gz.sha256
```

**Recommended:** Using [scripts/cross-build.sh](scripts/cross-build.sh) automatically generates archives and checksums.

---

## References

- [cross - Zero setup cross compilation](https://github.com/cross-rs/cross)
- [Rust Platform Support](https://doc.rust-lang.org/rustc/platform-support.html)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Cross-compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)
