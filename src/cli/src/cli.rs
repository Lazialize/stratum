// CLI Layer
// ユーザー入力の受付とコマンドルーティング

pub mod command_context;
pub mod commands;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// 出力フォーマット
#[derive(Clone, Debug, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output (default)
    #[default]
    Text,
    /// Structured JSON output
    Json,
}

/// Strata - Database Schema Management CLI
///
/// Infrastructure as Code for database schemas.
/// Manage database schema definitions as code with version control.
#[derive(Parser, Debug)]
#[command(name = "strata")]
#[command(author = "Strata Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Database schema management CLI tool")]
#[command(long_about = "Strata - Database Schema Management CLI

Infrastructure as Code for database schemas.
Manage database schema definitions as code with version control.

Strata helps you:
  • Define database schemas in declarative YAML files
  • Generate migration files automatically from schema changes
  • Apply and rollback migrations with confidence
  • Validate schema integrity before deployment
  • Export existing database schemas to code

Supported databases: PostgreSQL, MySQL, SQLite")]
#[command(propagate_version = true)]
#[command(after_help = "GETTING STARTED:
  1. Initialize a new project:     strata init --dialect sqlite
  2. Define your schema:            Edit files in schema/ directory
  3. Generate migrations:           strata generate
  4. Apply migrations:              strata apply
  5. Check migration status:        strata status

For detailed help on each command, use: strata <command> --help")]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Output format (text or json)
    #[arg(long, global = true, value_enum, default_value = "text")]
    pub format: OutputFormat,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new schema management project
    ///
    /// Creates the necessary directory structure and configuration files
    /// for managing database schemas with Strata.
    ///
    /// EXAMPLES:
    ///   # Initialize with SQLite
    ///   strata init --dialect sqlite
    ///
    ///   # Initialize with PostgreSQL
    ///   strata init --dialect postgresql
    ///
    ///   # Force re-initialization
    ///   strata init --force
    Init {
        /// Database dialect (postgresql, mysql, sqlite)
        #[arg(short, long, value_name = "DIALECT")]
        dialect: String,

        /// Force initialization even if config exists
        #[arg(short, long)]
        force: bool,

        /// Add .strata.yaml to .gitignore automatically
        #[arg(long)]
        add_gitignore: bool,
    },

    /// Generate migration files from schema changes
    ///
    /// Compares the current schema definition with the previous snapshot
    /// and generates migration files (up and down scripts) for any detected changes.
    ///
    /// EXAMPLES:
    ///   # Generate migration with description
    ///   strata generate --description "add user email column"
    ///
    ///   # Generate with auto-generated description
    ///   strata generate
    ///
    ///   # Dry run to preview SQL and type changes
    ///   strata generate --dry-run
    Generate {
        /// Description for the migration
        #[arg(short, long, value_name = "DESCRIPTION")]
        description: Option<String>,

        /// Dry run - show SQL without creating files
        #[arg(long)]
        dry_run: bool,

        /// Allow destructive changes (DROP, RENAME, ENUM removal)
        #[arg(long)]
        allow_destructive: bool,
    },

    /// Apply pending migrations to the database
    ///
    /// Executes all unapplied migrations in order, updating the database
    /// schema to match the current schema definition.
    ///
    /// EXAMPLES:
    ///   # Apply to development environment
    ///   strata apply
    ///
    ///   # Dry run to preview SQL
    ///   strata apply --dry-run
    ///
    ///   # Apply to production with timeout
    ///   strata apply --env production --timeout 30
    Apply {
        /// Dry run - show SQL without executing
        #[arg(long)]
        dry_run: bool,

        /// Target environment (development, staging, production)
        #[arg(short, long, value_name = "ENV", default_value = "development")]
        env: String,

        /// Timeout for database operations (in seconds)
        #[arg(long, value_name = "SECONDS")]
        timeout: Option<u64>,

        /// Allow destructive changes (DROP, RENAME, ENUM removal)
        #[arg(long)]
        allow_destructive: bool,
    },

    /// Rollback applied migrations
    ///
    /// Reverts the most recently applied migration(s) by executing
    /// the down scripts.
    ///
    /// EXAMPLES:
    ///   # Rollback last migration
    ///   strata rollback
    ///
    ///   # Rollback last 3 migrations
    ///   strata rollback --steps 3
    ///
    ///   # Rollback in production
    ///   strata rollback --env production --steps 1
    ///
    ///   # Dry run to preview SQL
    ///   strata rollback --dry-run
    ///
    ///   # Allow destructive rollback
    ///   strata rollback --allow-destructive
    Rollback {
        /// Number of migrations to rollback
        #[arg(long, value_name = "N")]
        steps: Option<u32>,

        /// Target environment
        #[arg(short, long, value_name = "ENV", default_value = "development")]
        env: String,

        /// Dry run - show SQL without executing
        #[arg(long)]
        dry_run: bool,

        /// Allow destructive changes (DROP, RENAME, etc.)
        #[arg(long)]
        allow_destructive: bool,
    },

    /// Validate schema definitions
    ///
    /// Checks schema definition files for syntax errors, referential integrity,
    /// naming convention violations, and other potential issues.
    ///
    /// EXAMPLES:
    ///   # Validate default schema directory
    ///   strata validate
    ///
    ///   # Validate specific directory
    ///   strata validate --schema-dir ./custom-schema
    Validate {
        /// Path to schema directory
        #[arg(short, long, value_name = "DIR")]
        schema_dir: Option<PathBuf>,
    },

    /// Show migration status
    ///
    /// Displays information about applied and pending migrations,
    /// current schema version, and any drift between the schema
    /// definition and the actual database.
    ///
    /// EXAMPLES:
    ///   # Show status for development
    ///   strata status
    ///
    ///   # Show status for production
    ///   strata status --env production
    Status {
        /// Target environment
        #[arg(short, long, value_name = "ENV", default_value = "development")]
        env: String,
    },

    /// Export existing database schema to code
    ///
    /// Reads the current database schema structure and generates
    /// schema definition files in YAML format.
    ///
    /// EXAMPLES:
    ///   # Export to default schema directory
    ///   strata export
    ///
    ///   # Export to custom directory
    ///   strata export --output ./exported-schema
    ///
    ///   # Export from production
    ///   strata export --env production --output ./prod-schema
    ///
    ///   # Overwrite existing files
    ///   strata export --force
    Export {
        /// Output directory for schema files
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,

        /// Target environment
        #[arg(short, long, value_name = "ENV", default_value = "development")]
        env: String,

        /// Overwrite existing files without confirmation
        #[arg(long)]
        force: bool,

        /// Split output into separate YAML files per table (e.g., users.yaml, posts.yaml)
        #[arg(long)]
        split: bool,

        /// Export only specified tables (comma-separated)
        #[arg(long, value_name = "TABLES", value_delimiter = ',')]
        tables: Vec<String>,

        /// Exclude specified tables from export (comma-separated)
        #[arg(long, value_name = "TABLES", value_delimiter = ',')]
        exclude_tables: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
