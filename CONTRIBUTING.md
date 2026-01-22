# Contributing to Stratum

Thank you for your interest in contributing to Stratum! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Process](#development-process)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

### Our Pledge

We pledge to make participation in our project a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

### Our Standards

**Positive behavior includes:**
- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

**Unacceptable behavior includes:**
- Trolling, insulting/derogatory comments, and personal attacks
- Public or private harassment
- Publishing others' private information without permission
- Other conduct which could reasonably be considered inappropriate

### Enforcement

Project maintainers are responsible for clarifying standards and may take appropriate action in response to unacceptable behavior.

## Getting Started

### Prerequisites

- Rust 1.92 or later
- Git
- SQLite (for running tests)
- Docker (optional, for PostgreSQL/MySQL integration tests)

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork:

```bash
git clone https://github.com/YOUR_USERNAME/stratum.git
cd stratum
```

3. Add the upstream remote:

```bash
git remote add upstream https://github.com/Lazialize/stratum.git
```

### Build and Test

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run tests with ignored tests (requires Docker)
cargo test -- --ignored
```

## Development Process

### Branching Strategy

- `main` - Stable release branch
- `develop` - Development branch (if applicable)
- `feature/your-feature-name` - Feature branches
- `fix/your-fix-name` - Bug fix branches

### Workflow

1. **Create a branch** for your work:

```bash
git checkout -b feature/your-feature-name
```

2. **Make your changes** following our coding standards

3. **Write tests** for your changes (TDD approach preferred)

4. **Run tests** to ensure everything passes:

```bash
cargo test
cargo clippy
cargo fmt --check
```

5. **Commit your changes** with descriptive messages:

```bash
git commit -m "feat: add support for custom migration directories"
```

6. **Push to your fork**:

```bash
git push origin feature/your-feature-name
```

7. **Open a Pull Request** on GitHub

### Commit Message Format

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `test` - Adding or updating tests
- `refactor` - Code refactoring
- `perf` - Performance improvements
- `chore` - Build process or tooling changes

**Examples:**
```
feat(cli): add --dry-run flag to apply command

Allows users to preview SQL without executing migrations.

Closes #123
```

```
fix(parser): handle null values in YAML schema

Fixed panic when parsing schema with explicit null values.
```

## Pull Request Process

### Before Submitting

- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation is updated (if applicable)
- [ ] CHANGELOG.md is updated (for significant changes)
- [ ] Commit messages follow conventions

### PR Description

Your PR description should include:

1. **Summary** - Brief description of changes
2. **Motivation** - Why is this change needed?
3. **Changes** - List of specific changes made
4. **Testing** - How did you test these changes?
5. **Screenshots** - If applicable (for UI/output changes)

### Review Process

1. At least one maintainer must approve the PR
2. All CI checks must pass
3. Discussion and requested changes must be addressed
4. Once approved, a maintainer will merge the PR

### After Merge

1. Delete your feature branch
2. Update your local repository:

```bash
git checkout main
git pull upstream main
```

## Coding Standards

### Rust Style Guide

Follow the official [Rust Style Guide](https://doc.rust-lang.org/stable/style-guide/):

- Use `rustfmt` for code formatting
- Use `clippy` for linting
- Maximum line length: 100 characters
- Use idiomatic Rust patterns

### Code Organization

```
src/
  ├── cli/              # CLI layer
  │   ├── commands/     # Command handlers
  │   └── mod.rs
  ├── core/             # Domain models
  ├── services/         # Business logic
  ├── adapters/         # External integrations
  └── lib.rs
```

### Error Handling

- Use `anyhow::Result` for application errors
- Use `thiserror` for custom error types
- Provide context with `.context()` or `.with_context()`
- Write user-friendly error messages

**Example:**
```rust
use anyhow::{Context, Result};

pub fn read_config(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .context(format!("設定ファイルの読み込みに失敗しました: {}", path.display()))?;

    serde_yaml::from_str(&contents)
        .context("設定ファイルのパースに失敗しました")
}
```

### Documentation

- Add doc comments to all public items
- Include examples in doc comments
- Use `///` for item documentation
- Use `//!` for module documentation

**Example:**
```rust
/// スキーマ定義を検証します。
///
/// # 引数
///
/// * `schema` - 検証対象のスキーマ
///
/// # 戻り値
///
/// 検証が成功した場合は `Ok(())`、失敗した場合は検証エラーを返します。
///
/// # 例
///
/// ```
/// use stratum::services::SchemaValidator;
///
/// let validator = SchemaValidator::new();
/// let result = validator.validate(&schema);
/// assert!(result.is_ok());
/// ```
pub fn validate(&self, schema: &Schema) -> Result<()> {
    // Implementation
}
```

## Testing Guidelines

### Test-Driven Development

We follow TDD principles:

1. **Write a failing test** - Define expected behavior
2. **Implement the minimum** - Make the test pass
3. **Refactor** - Improve code quality

### Test Organization

```
tests/
  ├── unit/                    # Unit tests (optional)
  ├── integration/             # Integration tests
  ├── schema_parser_test.rs    # Service tests
  ├── database_integration_test.rs
  └── ...
```

### Test Naming

Use descriptive test names that explain what is being tested:

```rust
#[test]
fn test_schema_parser_returns_error_for_invalid_yaml() {
    // Test implementation
}

#[tokio::test]
async fn test_apply_command_creates_migration_history_table() {
    // Test implementation
}
```

### Test Coverage

- Aim for high test coverage (>80%)
- Test edge cases and error conditions
- Use `#[ignore]` for tests requiring external resources (Docker)
- Mock external dependencies when appropriate

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_schema_parser

# Ignored tests (Docker required)
cargo test -- --ignored

# With output
cargo test -- --nocapture

# Release mode
cargo test --release
```

## Documentation

### README Updates

When adding new features, update:

- Feature list
- Quick start guide (if applicable)
- Command examples
- Configuration options

### Code Documentation

- Add rustdoc comments to public APIs
- Include usage examples
- Document panics, errors, and safety concerns

### CHANGELOG

For significant changes, add an entry to `CHANGELOG.md`:

```markdown
### Added
- New feature description

### Changed
- Changed behavior description

### Fixed
- Bug fix description
```

## Community

### Getting Help

- **GitHub Issues** - Bug reports and feature requests
- **Discussions** - Questions and general discussion
- **Pull Requests** - Code contributions

### Reporting Bugs

When reporting bugs, include:

1. **Description** - Clear description of the issue
2. **Steps to Reproduce** - Minimal reproduction steps
3. **Expected Behavior** - What should happen
4. **Actual Behavior** - What actually happens
5. **Environment** - OS, Rust version, database version
6. **Logs** - Relevant error messages or logs

**Example:**
```markdown
## Bug Report

**Description**
Migration fails when table name contains hyphens.

**Steps to Reproduce**
1. Create schema with table name `user-profiles`
2. Run `stratum generate`
3. Run `stratum apply`

**Expected Behavior**
Migration should apply successfully.

**Actual Behavior**
Error: Invalid table name syntax.

**Environment**
- OS: macOS 14.0
- Rust: 1.92.0
- Database: PostgreSQL 16.1

**Logs**
```
Error: Invalid table name syntax: user-profiles
```
```

### Requesting Features

For feature requests, include:

1. **Use Case** - Why is this feature needed?
2. **Proposed Solution** - How should it work?
3. **Alternatives** - Other approaches considered
4. **Additional Context** - Screenshots, examples, etc.

## Development Tips

### Hot Reload

Use `cargo watch` for automatic rebuilds:

```bash
cargo install cargo-watch
cargo watch -x check -x test
```

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo run -- apply
```

### Performance Profiling

Use `cargo flamegraph` for profiling:

```bash
cargo install flamegraph
cargo flamegraph --bin stratum
```

### Cross-Compilation

See [BUILDING.md](BUILDING.md) for cross-compilation instructions.

## License

By contributing to Stratum, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).

## Questions?

If you have questions about contributing, feel free to:

- Open a GitHub Discussion
- Ask in a Pull Request
- Open an issue for clarification

Thank you for contributing to Stratum! 🚀
