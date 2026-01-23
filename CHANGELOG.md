# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of Strata - Database Schema Management CLI
- Schema definition in declarative YAML format
- Automatic migration generation from schema changes
- Multi-database support (PostgreSQL, MySQL, SQLite)
- Complete CLI with 7 commands:
  - `init` - Initialize new schema management project
  - `generate` - Generate migration files from schema changes
  - `apply` - Apply pending migrations to database
  - `rollback` - Rollback applied migrations
  - `validate` - Validate schema definitions
  - `status` - Show migration status
  - `export` - Export existing database schema to code
- Test-Driven Development with comprehensive test coverage (84 tests)
- Database integration tests with testcontainers
- Cross-compilation support for Linux, macOS, and Windows
- Automated release build scripts
- Detailed documentation (README.md, BUILDING.md)

### Core Features

#### Schema Management
- YAML-based schema definitions with version control
- Support for tables, columns, indexes, and constraints
- Schema validation with referential integrity checks
- Checksum-based migration integrity verification

#### Migration System
- Automatic migration file generation (up/down SQL)
- Timestamp-based migration versioning
- Transaction-based migration application
- Rollback support with down migrations
- Migration history tracking

#### SQL Generation
- PostgreSQL SQL generator with full type support
- MySQL SQL generator with dialect-specific features
- SQLite SQL generator with limitation handling
- Schema diff detection for migration generation

#### Database Support
- PostgreSQL (tested with PostgreSQL 16)
- MySQL (tested with MySQL 8.0+)
- SQLite (tested with SQLite 3.x)
- Unified database interface with SQLx

### Documentation
- Comprehensive README with quick start guide
- Detailed build instructions (BUILDING.md)
- CLI help text with usage examples
- Configuration file documentation
- Troubleshooting guide

### Development
- Test coverage: 84 passing tests
- Unit tests for all core components
- Integration tests with SQLite
- Docker-based integration tests for PostgreSQL
- Continuous integration ready

### Build & Release
- Optimized release profile (LTO, strip, opt-level=3)
- Binary size: ~571KB (aarch64-apple-darwin)
- Cross-compilation configuration for:
  - Linux (x86_64, ARM64, musl)
  - macOS (Intel, Apple Silicon)
  - Windows (MinGW, MSVC)
- Universal binary support for macOS
- Automated release build script

## [0.1.0] - 2026-01-22

### Initial Development

This is the first development version of Strata, implementing the core schema management functionality.

#### Implemented Components

**CLI Layer**
- Command-line argument parsing with clap
- Global options (--config, --verbose, --no-color)
- Seven subcommands with detailed help text

**Domain Models**
- Schema, Table, Column, Index, Constraint types
- Migration and MigrationStatus types
- Custom error types with thiserror
- Serde serialization/deserialization

**Services**
- SchemaParserService - YAML schema parsing
- SchemaValidatorService - Schema validation
- SchemaChecksumService - SHA-256 checksum calculation
- SchemaDiffDetectorService - Schema comparison
- MigrationGeneratorService - Migration file generation
- DatabaseConnectionService - Database connection management
- DatabaseMigratorService - Migration execution

**SQL Generators**
- PostgresSqlGenerator - PostgreSQL DDL generation
- MysqlSqlGenerator - MySQL DDL generation
- SqliteSqlGenerator - SQLite DDL generation

**Command Handlers**
- InitCommandHandler - Project initialization
- GenerateCommandHandler - Migration generation
- ApplyCommandHandler - Migration application
- RollbackCommandHandler - Migration rollback
- ValidateCommandHandler - Schema validation
- StatusCommandHandler - Migration status display
- ExportCommandHandler - Schema export

**Tests**
- 84 unit and integration tests
- Test coverage across all layers
- Mock-free integration tests with SQLite
- Docker-based PostgreSQL tests (optional)

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

[Unreleased]: https://github.com/Lazialize/strata/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Lazialize/strata/releases/tag/v0.1.0
