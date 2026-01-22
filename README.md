# Stratum

> Database schema management CLI tool - Infrastructure as Code for database schemas

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)

Stratum is a modern database schema management tool that treats your database schema as code. Define your schemas in declarative YAML files, automatically generate migrations, and apply them with confidence across multiple environments.

## Features

- **📝 Schema as Code**: Define database schemas in declarative YAML files
- **🔄 Automatic Migration Generation**: Generate migration files from schema changes
- **✅ Schema Validation**: Validate integrity, foreign keys, and naming conventions
- **🔍 Migration Status Tracking**: Track applied and pending migrations
- **⬆️ Apply & Rollback**: Apply migrations or rollback with confidence
- **📤 Schema Export**: Export existing database schemas to code
- **🗄️ Multi-Database Support**: PostgreSQL, MySQL, and SQLite

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/Lazialize/stratum.git
cd stratum

# Build and install
cargo build --release
cargo install --path .
```

For detailed build instructions, cross-compilation, and platform-specific guides, see [BUILDING.md](BUILDING.md).

### Using Cargo

```bash
cargo install stratum
```

## Quick Start

### 1. Initialize a New Project

```bash
# Initialize with SQLite
stratum init --dialect sqlite

# Or with PostgreSQL
stratum init --dialect postgresql

# Or with MySQL
stratum init --dialect mysql
```

This creates:
- `.stratum.yaml` - Configuration file
- `schema/` - Directory for schema definitions
- `migrations/` - Directory for generated migrations

### 2. Define Your Schema

Create a schema file in the `schema/` directory (e.g., `schema/users.yaml`):

```yaml
version: "1.0"
tables:
  users:
    name: users
    columns:
      - name: id
        type:
          kind: INTEGER
          precision: null
        nullable: false
        default_value: null
        auto_increment: true
      - name: email
        type:
          kind: VARCHAR
          length: 255
        nullable: false
        default_value: null
        auto_increment: null
      - name: created_at
        type:
          kind: TIMESTAMP
          with_time_zone: true
        nullable: false
        default_value: null
        auto_increment: null
    indexes:
      - name: idx_users_email
        columns:
          - email
        unique: true
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
```

### 3. Generate Migration

```bash
# Generate migration with description
stratum generate --description "create users table"

# Or with auto-generated description
stratum generate
```

This creates a migration in `migrations/` directory with:
- `up.sql` - SQL to apply the migration
- `down.sql` - SQL to rollback the migration
- `.meta.yaml` - Migration metadata (version, checksum)

### 4. Apply Migration

```bash
# Apply to development environment
stratum apply

# Dry run to preview SQL
stratum apply --dry-run

# Apply to production
stratum apply --env production
```

### 5. Check Status

```bash
# Show migration status
stratum status

# Check production status
stratum status --env production
```

## Commands

### `init` - Initialize Project

Initialize a new schema management project.

```bash
# Basic initialization
stratum init --dialect sqlite

# Force re-initialization
stratum init --force
```

**Options:**
- `-d, --dialect <DIALECT>` - Database dialect (postgresql, mysql, sqlite)
- `-f, --force` - Force initialization even if config exists

### `generate` - Generate Migrations

Generate migration files from schema changes.

```bash
# With description
stratum generate --description "add user email column"

# Auto-generate description
stratum generate
```

**Options:**
- `-d, --description <DESCRIPTION>` - Description for the migration

### `apply` - Apply Migrations

Apply pending migrations to the database.

```bash
# Apply to development
stratum apply

# Dry run (preview only)
stratum apply --dry-run

# Apply to production with timeout
stratum apply --env production --timeout 30
```

**Options:**
- `--dry-run` - Show SQL without executing
- `-e, --env <ENV>` - Target environment (default: development)
- `--timeout <SECONDS>` - Timeout for database operations

### `rollback` - Rollback Migrations

Rollback applied migrations.

```bash
# Rollback last migration
stratum rollback

# Rollback last 3 migrations
stratum rollback --steps 3

# Rollback in production
stratum rollback --env production --steps 1
```

**Options:**
- `--steps <N>` - Number of migrations to rollback
- `-e, --env <ENV>` - Target environment (default: development)

### `validate` - Validate Schema

Validate schema definition files.

```bash
# Validate default schema directory
stratum validate

# Validate specific directory
stratum validate --schema-dir ./custom-schema
```

**Options:**
- `-s, --schema-dir <DIR>` - Path to schema directory

### `status` - Show Migration Status

Display migration status information.

```bash
# Show status for development
stratum status

# Show status for production
stratum status --env production
```

**Options:**
- `-e, --env <ENV>` - Target environment (default: development)

### `export` - Export Schema

Export existing database schema to code.

```bash
# Export to default schema directory
stratum export

# Export to custom directory
stratum export --output ./exported-schema

# Export from production
stratum export --env production --output ./prod-schema

# Overwrite existing files
stratum export --force
```

**Options:**
- `-o, --output <DIR>` - Output directory for schema files
- `-e, --env <ENV>` - Target environment (default: development)
- `--force` - Overwrite existing files without confirmation

## Configuration

The `.stratum.yaml` configuration file defines database connections and project settings.

### Example Configuration

```yaml
version: "1.0"
dialect: postgresql  # postgresql, mysql, or sqlite
schema_dir: schema
migrations_dir: migrations

environments:
  development:
    host: localhost
    port: 5432
    database: myapp_dev
    user: developer
    password: devpass
    timeout: 30

  production:
    host: db.example.com
    port: 5432
    database: myapp_prod
    user: app_user
    password: ${DB_PASSWORD}  # Use environment variable
    timeout: 60
```

### Configuration Fields

- `version` - Configuration file version
- `dialect` - Database type (postgresql, mysql, sqlite)
- `schema_dir` - Directory for schema definition files
- `migrations_dir` - Directory for migration files
- `environments` - Database connection settings per environment
  - `host` - Database host
  - `port` - Database port
  - `database` - Database name (or file path for SQLite)
  - `user` - Database user (optional for SQLite)
  - `password` - Database password (optional for SQLite)
  - `timeout` - Connection timeout in seconds

## Schema Definition Format

Stratum uses YAML for schema definitions. Each table is defined with its columns, indexes, and constraints.

### Column Types

Supported column types:

- `INTEGER` - Integer numbers
  - `precision`: Optional bit size
- `VARCHAR` - Variable-length strings
  - `length`: Maximum length (required)
- `TEXT` - Long text
- `BOOLEAN` - Boolean values
- `TIMESTAMP` - Date and time
  - `with_time_zone`: Optional timezone support
- `JSON` - JSON data

### Constraints

Supported constraints:

- `PRIMARY_KEY` - Primary key constraint
  - `columns`: List of column names
- `FOREIGN_KEY` - Foreign key constraint
  - `columns`: List of column names
  - `referenced_table`: Referenced table name
  - `referenced_columns`: Referenced column names
- `UNIQUE` - Unique constraint
  - `columns`: List of column names

### Example Schema

```yaml
version: "1.0"
tables:
  posts:
    name: posts
    columns:
      - name: id
        type:
          kind: INTEGER
        nullable: false
        auto_increment: true
      - name: title
        type:
          kind: VARCHAR
          length: 200
        nullable: false
      - name: content
        type:
          kind: TEXT
        nullable: true
      - name: user_id
        type:
          kind: INTEGER
        nullable: false
      - name: published
        type:
          kind: BOOLEAN
        nullable: false
        default_value: "false"
      - name: published_at
        type:
          kind: TIMESTAMP
          with_time_zone: true
        nullable: true
    indexes:
      - name: idx_posts_user_id
        columns:
          - user_id
        unique: false
      - name: idx_posts_published_at
        columns:
          - published_at
        unique: false
    constraints:
      - type: PRIMARY_KEY
        columns:
          - id
      - type: FOREIGN_KEY
        columns:
          - user_id
        referenced_table: users
        referenced_columns:
          - id
```

## Migration Files

Migration files are generated in the `migrations/` directory with the following structure:

```
migrations/
  20260122120000_create_users/
    up.sql         # SQL to apply the migration
    down.sql       # SQL to rollback the migration
    .meta.yaml     # Migration metadata
```

### Metadata File

The `.meta.yaml` file contains:

```yaml
version: "20260122120000"
description: "create_users"
checksum: "abc123def456..."  # SHA-256 hash of up.sql
```

The checksum ensures migration integrity - any modification to the migration after it's been applied will be detected.

## Best Practices

### 1. Schema Organization

- **One table per file**: Create separate YAML files for each table
- **Meaningful names**: Use descriptive names for tables, columns, and constraints
- **Consistent naming**: Follow naming conventions (e.g., snake_case)

### 2. Migration Workflow

1. **Always validate** before generating migrations: `stratum validate`
2. **Review generated SQL** before applying: `stratum apply --dry-run`
3. **Test in development** before production
4. **Never modify** applied migrations - create new ones instead
5. **Keep migrations small** - one logical change per migration

### 3. Environment Management

- Use **environment-specific configurations** for different databases
- Store **sensitive credentials** in environment variables
- Test migrations in **staging** before production
- Maintain **separate databases** for each environment

### 4. Version Control

- **Commit schema files** to version control
- **Commit migration files** to version control
- **Don't commit** `.stratum.yaml` if it contains sensitive data
- Use **`.gitignore`** for database files and credentials

## Troubleshooting

### Migration Checksum Mismatch

If you see a checksum mismatch warning:

1. **Don't modify applied migrations** - create a new migration instead
2. Check if someone modified the migration file after it was applied
3. If intentional, you may need to manually update the database migration history

### Connection Timeout

If database connections timeout:

1. Increase timeout in `.stratum.yaml`
2. Check network connectivity
3. Verify database credentials
4. Ensure database is running

### Validation Errors

If schema validation fails:

1. Check error message for specific issues
2. Verify all referenced tables exist
3. Ensure foreign key columns match referenced columns
4. Check naming conventions

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/Lazialize/stratum.git
cd stratum

# Build
cargo build

# Run tests
cargo test

# Build release version
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under either of:

- MIT license ([LICENSE](LICENSE) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- CLI powered by [clap](https://github.com/clap-rs/clap)
- Database access via [SQLx](https://github.com/launchbadge/sqlx)
- YAML parsing with [serde-saphyr](https://github.com/Ethiraric/saphyr)

---

Made with ❤️ by the Stratum Contributors
