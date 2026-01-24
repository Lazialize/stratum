# Project Structure Steering

## Source Organization

### Primary Directory Layout

```
stratum/
├── src/                    # Source code (lib + binary)
│   ├── lib.rs             # Library entry point
│   ├── main.rs            # CLI binary entry point
│   ├── cli/               # Command layer
│   ├── core/              # Domain models & logic
│   ├── services/          # Business logic orchestration
│   └── adapters/          # External system integration
├── tests/                  # Integration tests
├── example/                # Example schema files
│   └── schema/            # Sample YAML schemas
├── .kiro/                  # Kiro AI-assisted development
│   ├── specs/             # Feature specifications (TDD/BDD)
│   └── steering/          # Project knowledge (this file)
├── Cargo.toml             # Rust package manifest
├── README.md              # User-facing documentation
├── BUILDING.md            # Build instructions
└── CLAUDE.md              # AI development guidelines
```

### Module Hierarchy

#### CLI Layer (`src/cli/`)
- **Purpose**: User interface, command parsing, output formatting
- **Pattern**: One file per command (e.g., `init.rs`, `generate.rs`, `apply.rs`)
- **Key files**:
  - `cli.rs`: CLI app definition with clap
  - `commands/mod.rs`: Command registry
  - `commands/{command}.rs`: Individual command handlers (init/generate/apply/validate/status/export/rollback)

#### Core Domain (`src/core/`)
- **Purpose**: Business entities, domain logic, validation rules
- **Pattern**: Pure Rust data structures with serde serialization
- **Key files**:
  - `schema.rs`: Schema models (`Table`, `Column`, `ColumnType`, `Constraint`)
  - `migration.rs`: Migration metadata (`Migration`, `MigrationVersion`)
  - `config.rs`: Configuration models (`.strata.yaml`)
  - `schema_diff.rs`: Diff representation (`SchemaDiff`, `TableChange`)
  - `error.rs`: Domain error types (`SchemaError`, `ValidationError`)
  - `type_category.rs`: Column type categorization
  - `naming.rs`: Naming rules and helpers

#### Services Layer (`src/services/`)
- **Purpose**: Business logic orchestration, multi-step workflows
- **Pattern**: Services operate on core models, coordinate adapters
- **Key files**:
  - `schema_parser.rs`: YAML → Schema conversion
  - `schema_validator.rs`: Integrity checks (foreign keys, constraints)
  - `schema_diff_detector.rs`: Schema comparison logic
  - `migration_generator.rs`: Migration file generation
  - `schema_checksum.rs`: SHA-256 checksums for integrity
  - `schema_serializer.rs`: Schema serialization utilities
  - `schema_conversion.rs`: Schema format conversion
  - `migration_pipeline.rs`: End-to-end migration workflow
  - `type_change_validator.rs`: Column type change safety checks
  - `dto.rs`: Service layer data transfer objects
  - `dto_converter.rs`: DTO ↔ domain model conversion

#### Adapters Layer (`src/adapters/`)
- **Purpose**: External system integration (databases, file system)
- **Pattern**: Trait-based abstractions for testability
- **Key files**:
  - `database.rs`: Database connection management (sqlx pools)
  - `database_migrator.rs`: Execute migrations against DB
  - `database_introspector.rs`: Existing schema inspection
  - `type_mapping.rs`: Dialect type mapping utilities
  - `sql_generator/`: Dialect-specific SQL generation
    - `postgres.rs`: PostgreSQL SQL generator
    - `mysql.rs`: MySQL SQL generator
    - `sqlite.rs`: SQLite SQL generator
    - `sqlite_table_recreator.rs`: SQLite table recreation workflow

### Testing Structure

#### Integration Tests (`tests/`)
- **Pattern**: One test file per major feature/component
- **Examples**:
  - `schema_model_test.rs`: Schema serialization/deserialization
  - `postgres_sql_generator_test.rs`: PostgreSQL SQL generation
  - `schema_validator_test.rs`: Validation logic
  - `database_integration_test.rs`: Real database operations (with testcontainers)

#### Unit Tests
- **Location**: Inline `#[cfg(test)]` modules in source files
- **Pattern**: Test public APIs and edge cases
- **Coverage**: 152+ unit tests across core/services/adapters

## File Naming Conventions

### Source Files
- **Module files**: `snake_case.rs` (e.g., `schema_validator.rs`)
- **Test files**: `{feature}_test.rs` (e.g., `migration_generator_test.rs`)
- **Module index**: `mod.rs` (re-exports public items)

### Schema Files (User-facing)
- **Location**: `schema/` directory (configurable via `.strata.yaml`)
- **Format**: `{table_name}.yaml` (e.g., `users.yaml`, `posts.yaml`)
- **Requirement**: Must be valid YAML with `version: "1.0"` and `tables` key

### Migration Files
- **Location**: `migrations/` directory (configurable)
- **Structure**:
  ```
  migrations/
    {version}_{description}/
      up.sql        # Forward migration
      down.sql      # Rollback migration
      .meta.yaml    # Metadata (version, checksum, timestamp)
  ```
- **Version format**: `YYYYMMDDHHMMSS` (e.g., `20260122120000`)

## Import Conventions

### Internal Imports
```rust
// Prefer absolute paths from crate root
use crate::core::schema::{Schema, Table, ColumnType};
use crate::services::schema_validator::SchemaValidatorService;
use crate::adapters::sql_generator::postgres::PostgresGenerator;
```

### External Imports
```rust
// Group by category
// 1. Standard library
use std::path::PathBuf;
use std::collections::HashMap;

// 2. External crates
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// 3. Crate modules (as above)
```

### Re-exports
- `mod.rs` files re-export public types
- Example (`src/core/mod.rs`):
  ```rust
  pub mod schema;
  pub mod migration;
  pub mod config;
  pub mod error;

  pub use schema::{Schema, Table, Column};
  pub use migration::Migration;
  pub use config::Config;
  ```

## Configuration Files

### Project Configuration
- **`.strata.yaml`**: User-facing config (generated by `strata init`)
  - Database connection settings per environment
  - Schema/migration directory paths
  - Dialect selection (postgresql, mysql, sqlite)

### Development Configuration
- **`Cargo.toml`**: Rust package manifest
  - Dependencies with explicit versions
  - Release profile optimizations
  - Metadata (description, license, keywords)

### AI Development
- **`.kiro/specs/{feature}/`**: Feature specifications (requirements, design, tasks)
- **`CLAUDE.md`**: AI collaboration guidelines (TDD, spec-driven development)

## Example Files

### Purpose
- Located in `example/schema/`
- Demonstrate schema definition patterns
- Used for manual testing and documentation

### Current Examples
- `users.yaml`: Basic table with constraints
- `products.yaml`: Comprehensive example with all 15 data types
- `dialect_specific_example.yaml`: 方言別の型・制約の差分を示す例

### Dialect-Specific Examples
- `*_specific_types.yml`: 方言固有の型マッピングを単独で示す例
- `migrations/`: サンプルのマイグレーション構成（検証や説明用途）

## Directory Exclusions

### Version Control (`.gitignore`)
- Build artifacts: `target/`, `Cargo.lock`
- IDE files: `.vscode/`, `.idea/`
- Local logs/snapshots: `.strata/logs/`, `.strata/snapshots/`
- Distribution artifacts: `dist/`

### Documentation Exclusions
- Temporary files: `.DS_Store`, `*.swp`
- Dependencies: `target/` (contains generated code)
- Agent tooling: `.cursor/`, `.gemini/` (agent-specific, not project knowledge)

## Organizational Principles

1. **Separation of Concerns**: CLI, Core, Services, Adapters have distinct responsibilities
2. **Dependency Flow**: Inward (CLI → Services → Core ← Adapters)
3. **Testability**: Core/Services are pure, Adapters are mockable via traits
4. **Feature Modularity**: Commands, generators, validators are independently testable
5. **Example-Driven**: Examples serve as both documentation and smoke tests

---

updated_at: 2026-01-25  
change_note: 追加サービス/アダプタ/コアのパターン、CLIコマンド例、`.strata`設定名を反映
