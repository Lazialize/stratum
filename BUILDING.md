# Building Strata

This document describes how to build Strata from source for different platforms.

## Prerequisites

- Rust 1.92 or later
- Cargo (comes with Rust)
- Platform-specific toolchains for cross-compilation (see below)

## Quick Start

### Development Build

For development and testing:

```bash
cargo build
```

The binary will be located at `target/debug/strata`.

### Release Build

For optimized production builds:

```bash
cargo build --release
```

The binary will be located at `target/release/strata`.

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

### Installing Cross-Compilation Targets

First, install the target toolchains:

```bash
# Linux targets
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-unknown-linux-musl  # Static binary, no glibc dependency
rustup target add aarch64-unknown-linux-gnu  # ARM64 Linux

# macOS targets
rustup target add x86_64-apple-darwin        # Intel Macs
rustup target add aarch64-apple-darwin       # Apple Silicon (M1/M2/M3)

# Windows targets
rustup target add x86_64-pc-windows-gnu      # MinGW
rustup target add x86_64-pc-windows-msvc     # MSVC
```

### Platform-Specific Linkers

#### Linux Cross-Compilation

On macOS or Linux, install cross-compilation toolchains:

```bash
# On macOS (using Homebrew)
brew install FiloSottile/musl-cross/musl-cross
brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu

# On Ubuntu/Debian
sudo apt-get install gcc-x86-64-linux-gnu
sudo apt-get install gcc-aarch64-linux-gnu
sudo apt-get install musl-tools
```

#### Windows Cross-Compilation

On macOS or Linux, install MinGW:

```bash
# On macOS (using Homebrew)
brew install mingw-w64

# On Ubuntu/Debian
sudo apt-get install mingw-w64
```

#### macOS Cross-Compilation

Building for macOS requires macOS and Xcode Command Line Tools:

```bash
xcode-select --install
```

### Building for Specific Targets

#### Linux x86_64 (GNU)

```bash
cargo build --release --target x86_64-unknown-linux-gnu
```

Binary location: `target/x86_64-unknown-linux-gnu/release/strata`

#### Linux x86_64 (musl - Static Binary)

```bash
cargo build --release --target x86_64-unknown-linux-musl
```

This creates a fully static binary with no glibc dependency, ideal for Docker containers or portable distributions.

Binary location: `target/x86_64-unknown-linux-musl/release/strata`

#### Linux ARM64

```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

Binary location: `target/aarch64-unknown-linux-gnu/release/strata`

#### macOS Intel (x86_64)

```bash
cargo build --release --target x86_64-apple-darwin
```

Binary location: `target/x86_64-apple-darwin/release/strata`

#### macOS Apple Silicon (ARM64)

```bash
cargo build --release --target aarch64-apple-darwin
```

Binary location: `target/aarch64-apple-darwin/release/strata`

#### Windows (MinGW)

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

Binary location: `target/x86_64-pc-windows-gnu/release/strata.exe`

#### Windows (MSVC)

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

Binary location: `target/x86_64-pc-windows-msvc/release/strata.exe`

## Universal Binaries (macOS)

To create a universal binary that runs on both Intel and Apple Silicon Macs:

```bash
# Build for both architectures
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine into universal binary
lipo -create \
  target/x86_64-apple-darwin/release/strata \
  target/aarch64-apple-darwin/release/strata \
  -output strata-universal

# Verify
lipo -info strata-universal
```

## Build Automation with cargo-make

For automated multi-platform builds, you can use `cargo-make`:

```bash
# Install cargo-make
cargo install cargo-make

# Create Makefile.toml with build tasks
# See project documentation for examples
```

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

## Troubleshooting

### Linker Errors

If you encounter linker errors during cross-compilation:

1. Ensure the target toolchain is installed: `rustup target list --installed`
2. Verify linker is in PATH: `which x86_64-linux-gnu-gcc`
3. Check `.cargo/config.toml` linker settings

### Missing Dependencies

Some platforms require platform-specific dependencies:

```bash
# Example: OpenSSL for Linux musl
brew install filosottile/musl-cross/musl-cross
brew install messense/macos-cross-toolchains/x86_64-unknown-linux-musl
```

### Large Binary Size

If binary size is larger than expected:

1. Verify `strip = true` in `[profile.release]`
2. Check for debug symbols: `file target/release/strata`
3. Use `cargo bloat` to analyze binary size:

```bash
cargo install cargo-bloat
cargo bloat --release
```

## CI/CD Integration

For automated builds in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
name: Build
on: [push]
jobs:
  build:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - x86_64-unknown-linux-musl
          - x86_64-apple-darwin
          - aarch64-apple-darwin
          - x86_64-pc-windows-gnu
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo build --release --target ${{ matrix.target }}
```

## Performance Profiling

To build with debug symbols for profiling:

```bash
cargo build --profile release-with-debug
```

This uses the `release-with-debug` profile defined in `.cargo/config.toml`.

## Testing Release Builds

Always test release builds before distribution:

```bash
# Build release
cargo build --release

# Run tests with release binary
cargo test --release

# Integration tests
cargo test --release --test database_integration_test
```

## Distribution

### Creating Archives

```bash
# Linux/macOS
tar -czf strata-linux-x86_64.tar.gz -C target/x86_64-unknown-linux-gnu/release strata

# Windows (using 7zip or similar)
7z a strata-windows-x86_64.zip target/x86_64-pc-windows-gnu/release/strata.exe
```

### Checksums

Generate checksums for distribution:

```bash
# SHA256
shasum -a 256 strata-linux-x86_64.tar.gz > strata-linux-x86_64.tar.gz.sha256

# MD5
md5sum strata-linux-x86_64.tar.gz > strata-linux-x86_64.tar.gz.md5
```

## References

- [Rust Platform Support](https://doc.rust-lang.org/nightly/rustc/platform-support.html)
- [Cargo Book - Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Cross-compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)
- [cargo-make Documentation](https://github.com/sagiegurari/cargo-make)
