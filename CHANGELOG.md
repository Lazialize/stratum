# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-nightly-20260125.1341] - 2026-01-25

### Initial Release

First release of Strata - Database Schema Management CLI Tool

### Features

#### Project Structure
- Reorganized root as virtual workspace with `src/cli`, `src/core`, and `src/db`
- Unified crate names to `strata-core` and `strata-db`
- Improved maintainability through modular design

#### Schema Management
- Schema definition in declarative YAML format
- Support for tables, columns, indexes, and constraints
- Schema validation with referential integrity checks
- Checksum-based migration integrity verification
- Version control integration

#### Migration System
- Automatic migration file generation from schema changes
- Automatic generation of up/down SQL
- Timestamp-based migration versioning
- Transaction-based migration application
- Rollback support with down migrations
- Migration history tracking

#### SQL Generation
- PostgreSQL SQL generator with full type support
- MySQL SQL generator with dialect-specific features
- SQLite SQL generator with proper limitation handling
- Schema diff detection for migration generation

#### Database Support
- PostgreSQL 12+ (tested with PostgreSQL 16)
- MySQL 8.0+
- SQLite 3.x
- Unified database interface with SQLx

#### CLI Commands
- `init` - Initialize new schema management project
- `generate` - Generate migration files from schema changes
- `apply` - Apply pending migrations to database
- `rollback` - Rollback applied migrations
- `validate` - Validate schema definitions
- `status` - Show migration status
- `export` - Export existing database schema to code

#### Documentation
- Comprehensive README with quick start guide
- Detailed build instructions (BUILDING.md)
- CLI help text with usage examples
- Configuration file documentation
- Troubleshooting guide
- Dialect-specific types documentation

#### Testing
- Test coverage: 84 passing tests
- Unit tests for all core components
- Integration tests with SQLite
- Docker-based integration tests for PostgreSQL
- CI/CD ready

#### Build & Release
- Optimized release profile (LTO, strip, opt-level=3)
- Binary size: ~571KB (aarch64-apple-darwin)
- Cross-compilation configuration for:
  - Linux (x86_64, ARM64, musl)
  - macOS (Intel, Apple Silicon)
  - Windows (MinGW, MSVC)
- Universal binary support for macOS
- Automated release build script
- Easy setup with installation script

### Implementation Components

#### CLI Layer
- Command-line argument parsing with clap
- Global options (--config, --verbose, --no-color)
- Seven subcommands with detailed help text

#### Domain Models
- Schema, Table, Column, Index, Constraint types
- Migration and MigrationStatus types
- Custom error types with thiserror
- Serde serialization/deserialization

#### Services
- SchemaParserService - YAML schema parsing
- SchemaValidatorService - Schema validation
- SchemaChecksumService - SHA-256 checksum calculation
- SchemaDiffDetectorService - Schema comparison
- MigrationGeneratorService - Migration file generation
- DatabaseConnectionService - Database connection management
- DatabaseMigratorService - Migration execution

#### SQL Generators
- PostgresSqlGenerator - PostgreSQL DDL generation
- MysqlSqlGenerator - MySQL DDL generation
- SqliteSqlGenerator - SQLite DDL generation

#### Command Handlers
- InitCommandHandler - Project initialization
- GenerateCommandHandler - Migration generation
- ApplyCommandHandler - Migration application
- RollbackCommandHandler - Migration rollback
- ValidateCommandHandler - Schema validation
- StatusCommandHandler - Migration status display
- ExportCommandHandler - Schema export

#### Dependencies

**Production**
- clap 4.5 - CLI framework
- tokio 1.49 - Async runtime
- sqlx 0.8 - Database toolkit
- serde 1.0 - Serialization framework
- serde-saphyr 0.0.16 - YAML parser
- anyhow 1 - Error handling
- thiserror 2 - Error types
- chrono 0.4 - Date/time handling
- sha2 0.10 - SHA-256 hashing
- regex 1 - Regular expressions
- colored 3.1 - Terminal colors
- indicatif 0.18 - Progress bars

**Development**
- tokio-test 0.4 - Async testing
- tempfile 3 - Temporary files
- testcontainers 0.26 - Container-based testing
- testcontainers-modules 0.14 - Database modules

#### Known Limitations

- No automatic schema introspection from running databases (export command provides manual export)
- Limited support for database-specific features (focuses on common SQL features)
- Mutation testing not included (planned for future releases)

#### Platform Support

**Tested Platforms**
- macOS (Intel and Apple Silicon)
- Linux x86_64 (via cross-compilation)
- SQLite (all platforms)

**Supported Databases**
- PostgreSQL 12+
- MySQL 8.0+
- SQLite 3.x

## Future Roadmap

### Planned for 0.2.0
- Web UI for schema visualization
- Migration plan preview with dependency graph
- Automated schema migration on CI/CD
- Database seeding support
- Enhanced error messages with suggestions

### Planned for 0.3.0
- Schema documentation generation
- Database snapshot and restore
- Multi-tenancy support
- Schema versioning with Git integration

### Long-term Goals
- Schema testing framework
- Performance optimization for large schemas
- Cloud database support (AWS RDS, Azure SQL, etc.)
- Terraform/Pulumi integration
- GraphQL schema generation

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- CLI powered by [clap](https://github.com/clap-rs/clap)
- Database access via [SQLx](https://github.com/launchbadge/sqlx)
- YAML parsing with [serde-saphyr](https://github.com/Ethiraric/saphyr)
- Testing with [testcontainers-rs](https://github.com/testcontainers/testcontainers-rs)

[0.1.0-nightly-20260125.1341]: https://github.com/Lazialize/strata/releases/tag/v0.1.0-nightly-20260125.1341
