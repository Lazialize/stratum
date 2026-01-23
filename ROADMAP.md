# Roadmap (TODO)

This file captures potential future features and improvements. Items are not in strict priority order.

## Safety and Reliability
- Preview schema diffs before generating migrations (table/column/constraint changes).
- Require explicit flags for destructive operations (drop, rename, enum removal).
- Validate migration plans for risky steps (lock time, index rebuild, data loss).
- Add checksum verification gates for production applies.

## Developer Experience
- Improve `strata status` output with diff summaries and hints.
- Support environment overrides in config (e.g., `.strata.yaml` + `.strata.<env>.yaml`).
- Provide clearer error messages with actionable suggestions.
- Add interactive prompts for initialization and dangerous operations.
- Add a VS Code Language Server for YAML schema files (completion, diagnostics, go-to-definition).

## Migration Planning
- Generate a migration plan graph with dependencies.
- Show ordered steps in dry-run with per-step SQL.
- Detect rename candidates for columns and tables.
- Allow manual approval steps in CI/CD.

## Schema Linting
- Enforce naming conventions (tables, columns, indexes).
- Suggest missing indexes for foreign keys.
- Warn on nullable foreign keys and ambiguous defaults.
- Detect inconsistent data types across dialects.

## Database Coverage
- Expand dialect-specific data type support.
- Improve enum support across dialects where possible.
- Capture database-specific constraints in export.

## Export and Introspection
- Improve export fidelity (defaults, constraints, ordering, comments).
- Round-trip parity tests (DB -> YAML -> DB).
- Include schema documentation output from existing databases.

## Data Seeding
- Support seed files per environment.
- Add idempotent seed execution with checksums.
- Allow seed ordering and dependencies.

## CI/CD Integration
- Provide GitHub Actions templates for validate/diff.
- Add a `strata check` command for CI (validate + diff + lint).
- Machine-readable output (JSON) for CI tooling.

## Documentation
- Add example schemas for advanced features (enums, composite keys).
- Provide migration best practices and safety guidelines.
- Add a troubleshooting guide with common errors.
