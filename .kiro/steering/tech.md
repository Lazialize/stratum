# Technical Steering

## Tech Stack

### Core Technologies
- **Language**: Rust 1.92+ (2021 edition)
- **Runtime**: Tokio 1.49 (async runtime for database operations)
- **Build System**: Cargo (standard Rust toolchain)

### Key Dependencies

#### CLI & User Interface
- **clap 4.5** (derive features): Type-safe CLI argument parsing with derive macros
- **colored 3.1**: Terminal color output for user feedback
- **indicatif 0.18**: Progress bars for long-running operations

#### Database Connectivity
- **sqlx 0.8**: Async database driver with compile-time query verification
  - Supports: PostgreSQL, MySQL, SQLite
  - Features: Connection pooling, migrations, type-safe queries

#### Data Processing
- **serde 1.x** (derive features): Serialization framework
- **serde-saphyr 0.0.16**: Panic-free YAML parser (chosen over serde_yaml)
- **sha2 0.10**: SHA-256 checksums for migration integrity

#### Error Handling
- **anyhow 1.x**: Application-level error context
- **thiserror 2.x**: Library-level custom error types

#### Utilities
- **chrono 0.4**: Timestamp handling with serde integration
- **regex 1.x**: Pattern matching for validation
- **async-trait 0.1**: Async trait definitions

### Development Dependencies
- **tokio-test 0.4**: Async testing utilities
- **tempfile 3.x**: Temporary file/directory creation for tests
- **testcontainers 0.26**: Container-based integration testing
- **testcontainers-modules 0.14**: PostgreSQL/MySQL test containers

## Architectural Decisions

### Layered Architecture (Clean Architecture)

```
┌─────────────────────────────────────────┐
│  CLI Layer (User Interface)             │  ← clap-based commands
├─────────────────────────────────────────┤
│  Services Layer (Business Logic)        │  ← Orchestration & validation
├─────────────────────────────────────────┤
│  Core Domain (Models & Logic)           │  ← Schema, Migration, Config models
├─────────────────────────────────────────┤
│  Adapters (External Integration)        │  ← Database, FileSystem access
└─────────────────────────────────────────┘
```

**Rationale**:
- Testable: Core domain is pure Rust, independent of I/O
- Flexible: Easy to swap database drivers or add new commands
- Clear boundaries: Dependencies flow inward (adapters depend on core, not vice versa)

### Async-First Design

- All database operations are async (via `tokio` + `sqlx`)
- Async traits for adapter abstractions (`#[async_trait]`)
- **Why**: Enables connection pooling, timeout handling, and future scalability

### Type-Safe Schema Modeling

- Enum-based `ColumnType` with tagged union serialization (`#[serde(tag = "kind")]`)
- Strongly typed `Dialect` enum (PostgreSQL, MySQL, SQLite)
- **Why**: Compile-time guarantees for schema validity, eliminating runtime type errors

### Panic-Free YAML Parsing

- Use `serde-saphyr` instead of `serde_yaml` (which can panic on malformed input)
- **Why**: CLI tools should never panic on user input; always return actionable errors

### Trait-Based SQL Generation

```rust
pub trait SqlGenerator {
    fn generate_create_table(&self, table: &Table) -> String;
    fn generate_alter_table(&self, diff: &SchemaDiff) -> Vec<String>;
    // ...
}
```

- Implementations: `PostgresGenerator`, `MySqlGenerator`, `SqliteGenerator`
- **Why**: Polymorphic handling of dialect-specific SQL generation

### Error Handling Strategy

1. **Library errors** (`thiserror`): Domain-specific errors with context
   - `SchemaError`, `ValidationError`, `MigrationError`
2. **Application errors** (`anyhow`): Top-level error aggregation
3. **User-facing errors**: Colored terminal output with suggestions

### Testing Strategy

- **Unit tests**: Inline `#[cfg(test)]` modules for business logic
- **Integration tests**: `tests/` directory with real database containers
- **Test coverage**: 152+ unit tests, 27+ test suites
- **Philosophy**: Test behavior, not implementation; focus on public APIs

## Development Workflow (`.github/instructions`)

### Core Development Principles
Based on `.github/instructions/rust-core.instructions.md`:

- **Small, reviewable changes**: Preserve existing style and public APIs
- **Idiomatic Rust first**: Ownership/borrowing before cloning
- **Avoid `unwrap()`/`expect()`**: Use proper error handling in library/production code
- **Test behavior changes**: Add tests for every functional change

### Pre-Commit Validation

**Required checks before committing**:
```bash
cargo fmt              # Format code
cargo clippy           # Lint and fix warnings
cargo test             # Run all tests
```

### Development Iteration Cycle
1. **Understand deeply**: Read issue, explore codebase, research online
2. **Plan incrementally**: Break into small testable steps
3. **Implement & test**: Small changes → run tests → iterate
4. **Debug rigorously**: Use `RUST_BACKTRACE=1`, `dbg!()`, and `cargo-expand`
5. **Validate comprehensively**: All tests pass + edge cases covered

### Common Anti-Patterns to Avoid
- Unnecessary `.clone()` instead of borrowing
- Overusing `.unwrap()`/`.expect()` (causes panics)
- Calling `.collect()` too early (prevents lazy iteration)
- Writing `unsafe` without clear need
- Over-abstracting with traits/generics
- Global mutable state (breaks testability)
- Heavy macro use (hides logic, hard to debug)
- Ignoring lifetime annotations
- Premature optimization

## Code Conventions

### Module Organization

```
src/
├── cli/                # CLIクレート (strata)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   └── cli/         # commands以下に各ハンドラー
│   └── tests/           # 統合テスト
├── core/               # ドメインクレート (strata-core)
│   └── src/core/        # スキーマ/マイグレーション/設定
└── db/                 # DBクレート (strata-db)
    └── src/
        ├── services/    # サービス層
        └── adapters/    # DB/SQL生成
```

### Naming Conventions

- **Modules**: snake_case (e.g., `schema_validator`)
- **Structs/Enums**: PascalCase (e.g., `ColumnType`, `SchemaDiff`)
- **Functions**: snake_case (e.g., `generate_migration`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_TIMEOUT`)

### Error Propagation

- Use `?` operator for error propagation
- Add context with `.with_context()` (anyhow)
- Custom error types for domain-specific failures

### Async Patterns

- Use `async fn` where I/O is involved
- Avoid blocking operations in async contexts
- Use `tokio::spawn` for concurrent tasks (if needed)

## Performance Considerations

### Release Build Optimizations

```toml
[profile.release]
opt-level = 3           # Maximum optimization
lto = true              # Link-Time Optimization
codegen-units = 1       # Single codegen unit for better optimization
strip = true            # Strip symbols to reduce binary size
panic = "abort"         # Faster panics (no unwinding)
```

### Connection Pooling

- `sqlx` connection pools with configurable timeout
- Reuse connections across migrations within a transaction

### Lazy Evaluation

- Schema files are parsed only when needed (not all upfront)
- Migration diffs computed on-demand

## Debugging & Tooling

### Debugging Techniques
- **Stack traces**: `RUST_BACKTRACE=1 cargo run`
- **Temporary logging**: `dbg!(variable)` macro
- **Structured logging**: `tracing` or `log` crates (when needed)
- **Macro expansion**: `cargo-expand` to debug derive macros

### Cargo Tools
- `cargo tree`: Explore dependency graph
- `cargo doc --open`: Generate and view documentation
- `cargo check`: Fast compile-time checks without codegen
- `cargo bench`: Benchmark performance (future)

### Testing Philosophy
- **Test public APIs**: Focus on behavior, not implementation
- **Edge cases first**: Boundary conditions, empty inputs, invalid data
- **Integration over unit**: Prefer end-to-end tests for CLI workflows
- **Testcontainers**: Use real databases for integration tests

## Security Principles

1. **No hardcoded credentials**: Use environment variables for sensitive data
2. **Parameterized queries**: `sqlx` provides compile-time query verification
3. **Input validation**: Validate YAML schema before processing
4. **Checksum integrity**: Detect tampering with applied migrations
5. **Panic-free error handling**: Never `unwrap()` on user input; always return `Result`

## Future Technical Directions

- **Plugins**: Extensible dialect support via dynamic loading
- **Parallel migrations**: Apply independent migrations concurrently
- **Query performance hints**: Analyze schema for index recommendations
- **Schema linting**: Enforce naming conventions and best practices
