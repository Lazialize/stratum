# Project Structure Steering

## Source Organization

### Primary Directory Layout

```
stratum/
├── Cargo.toml              # 仮想ワークスペースのマニフェスト
├── src/
│   ├── cli/                # CLIクレート (strata)
│   │   ├── Cargo.toml
│   │   ├── src/            # CLI実装 (lib/main/commands)
│   │   └── tests/          # CLI統合テスト
│   ├── core/               # ドメインクレート (strata-core)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── core/       # ドメインモデル/ロジック
│   └── db/                 # DBクレート (strata-db)
│       ├── Cargo.toml
│       └── src/
│           ├── adapters/   # 外部連携 (DB/SQL生成)
│           └── services/   # DB関連サービス
├── example/                # Example schema files
│   └── schema/             # Sample YAML schemas
├── resources/schemas/      # JSON Schema for YAML validation
├── scripts/                # Build automation (cross-build.sh)
├── dist/                   # Release binaries (gitignored)
├── .kiro/                  # Kiro AI-assisted development
│   ├── specs/              # Feature specifications (TDD/BDD)
│   └── steering/           # Project knowledge (this file)
├── README.md               # User-facing documentation
├── BUILDING.md             # Build instructions
├── CHANGELOG.md            # Version history
├── CONTRIBUTING.md         # Contribution guidelines
├── ROADMAP.md              # Future plans
└── CLAUDE.md               # AI development guidelines
```

### Module Hierarchy

#### CLIクレート (`src/cli/src/`)
- **Purpose**: CLIの入出力、コマンド解釈、表示整形
- **Pattern**: コマンド毎にファイル分割（例: `init.rs`, `generate.rs`, `apply.rs`）
- **Key files**:
  - `cli.rs`: clapベースのCLI定義
  - `commands/mod.rs`: コマンド登録とCommandOutput trait
  - `commands/{command}.rs`: 各コマンドハンドラー (init, generate, apply, rollback, status, validate, export)
  - `commands/command_context.rs`: コマンド共通コンテキスト
  - `commands/*_formatter.rs`: 出力整形（dry_run, destructive_change）

#### Coreクレート (`src/core/src/core/`)
- **Purpose**: ドメインモデル/ロジック（純粋なRust）
- **Pattern**: serde対応のデータ構造中心
- **Key files**:
  - `schema.rs`: スキーマモデル（`Table`, `Column`, `ColumnType`, `Constraint`）
  - `migration.rs`: マイグレーションモデル
  - `config.rs`: 設定モデル（`.strata.yaml`）
  - `schema_diff.rs`: スキーマ差分
  - `error.rs`: ドメインエラー
  - `destructive_change_report.rs`: 破壊的変更レポート
  - `naming.rs`: 命名規則ロジック
  - `type_category.rs`: 型カテゴリ分類

#### DBクレート (`src/db/src/`)
- **Purpose**: DB連携とDB関連サービスの集約
- **Pattern**: adapters/servicesで責務分離
- **Key modules**:
  - `adapters/sql_generator/`: 方言別SQL生成 (postgres, mysql, sqlite)
  - `adapters/database_introspector.rs`: 既存DBからのスキーマ取得（export用）
  - `adapters/database_migrator.rs`: マイグレーション実行
  - `adapters/type_mapping/`: 方言間の型マッピング (common, postgres, mysql, sqlite)
  - `adapters/connection_string.rs`: 接続文字列パース
  - `services/schema_validator/`: モジュール化された検証サブシステム (table, column_type, constraint, index, enum, dialect, rename)
  - `services/schema_diff_detector/`: スキーマ差分検出 (table, column, constraint, index, enum)
  - `services/schema_io/`: スキーマ読み書き (dto, parser, serializer)
  - `services/migration_pipeline/`: 多段マイグレーション生成 (table, enum, index_constraint)
  - `services/destructive_change_detector.rs`: 破壊的変更検出
  - `services/migration_generator.rs`: マイグレーション生成

### Testing Structure

#### Integration Tests (`src/cli/tests/`)
- **Pattern**: カテゴリプレフィックスによるファイル分割
- **Categories**:
  - `cmd_*.rs`: コマンド単位テスト (apply, export, generate, init, rollback, status, validate)
  - `e2e_*.rs`: エンドツーエンドテスト
  - `edge_*.rs`: エッジケーステスト (rename, constraint, enum, type_change等)
  - `gen_*.rs`: SQL生成テスト (postgres, mysql, sqlite, dialect_specific)
  - `integ_*.rs`: 統合テスト (cli, database, destructive_change, dialect_specific)
  - `model_*.rs`: モデルテスト (migration, schema)
  - `service_*.rs`: サービス層テスト (migrator, generator, checksum, diff, parser, validator)
  - `unit_*.rs`: ユニットテスト (cli_parsing, config, connection, dependencies, dialect等)

#### Unit Tests
- **Location**: Inline `#[cfg(test)]` modules in source files
- **Pattern**: Test public APIs and edge cases
- **Coverage**: 37テストファイル、カバレッジ94%

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
// クレート境界を明示する
use strata_core::core::schema::{Schema, Table, ColumnType};
use strata_db::services::schema_validator::SchemaValidatorService;
use strata_db::adapters::sql_generator::postgres::PostgresGenerator;
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
- `DIALECT_SPECIFIC_TYPES.md`: 方言固有型のドキュメント

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

updated_at: 2026-02-01
change_note: 新モジュール（破壊的変更検出、型マッピング、検証サブシステム等）、テストカテゴリ分類、resources/scripts/dist追加、ドキュメントファイル追加
