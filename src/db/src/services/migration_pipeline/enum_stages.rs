// ENUM関連パイプラインステージ
//
// PostgreSQLのENUM型の作成・変更・再作成を処理するステージ。

use crate::adapters::sql_generator::SqlGenerator;
use crate::core::schema_diff::EnumChangeKind;

use super::{MigrationPipeline, PipelineStageError};

impl<'a> MigrationPipeline<'a> {
    /// ステージ2: enum_statements (pre-table) - ENUM作成/変更
    pub(super) fn stage_enum_pre_table(
        &self,
        generator: &dyn SqlGenerator,
    ) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        // ENUM再作成の許可チェック
        if (!self.diff.removed_enums.is_empty()
            || self
                .diff
                .modified_enums
                .iter()
                .any(|e| matches!(e.change_kind, EnumChangeKind::Recreate)))
            && !self.allow_destructive
        {
            return Err(PipelineStageError::EnumRecreationNotAllowed);
        }

        // 新規ENUM作成
        for enum_def in &self.diff.added_enums {
            statements.extend(generator.generate_create_enum_type(enum_def));
        }

        // ENUM値追加（AddOnlyの場合）
        for enum_diff in &self.diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::AddOnly) {
                for value in &enum_diff.added_values {
                    statements
                        .extend(generator.generate_add_enum_value(&enum_diff.enum_name, value));
                }
            }
        }

        Ok(statements)
    }

    /// ステージ: enum_statements (post-table) - ENUM再作成
    pub(super) fn stage_enum_post_table(
        &self,
        generator: &dyn SqlGenerator,
    ) -> Result<Vec<String>, PipelineStageError> {
        let mut statements = Vec::new();

        for enum_diff in &self.diff.modified_enums {
            if matches!(enum_diff.change_kind, EnumChangeKind::Recreate) {
                statements.extend(generator.generate_recreate_enum_type(enum_diff));
            }
        }

        Ok(statements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Dialect;
    use crate::core::schema::EnumDefinition;
    use crate::core::schema_diff::{EnumColumnRef, EnumDiff, SchemaDiff};

    #[test]
    fn test_pipeline_enum_create() {
        let mut diff = SchemaDiff::new();
        diff.added_enums.push(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"CREATE TYPE "status" AS ENUM ('active', 'inactive')"#));
    }

    #[test]
    fn test_pipeline_enum_add_value() {
        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string()],
            new_values: vec!["active".to_string(), "inactive".to_string()],
            added_values: vec!["inactive".to_string()],
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::AddOnly,
            columns: Vec::new(),
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"ALTER TYPE "status" ADD VALUE 'inactive'"#));
    }

    #[test]
    fn test_pipeline_enum_recreate_requires_opt_in() {
        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string(), "inactive".to_string()],
            new_values: vec!["inactive".to_string(), "active".to_string()],
            added_values: Vec::new(),
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::Recreate,
            columns: Vec::new(),
        });

        let pipeline = MigrationPipeline::new(&diff, Dialect::PostgreSQL);
        let result = pipeline.generate_up();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage(), "enum_statements");
    }

    #[test]
    fn test_pipeline_enum_recreate_with_opt_in() {
        let mut diff = SchemaDiff::new();
        diff.modified_enums.push(EnumDiff {
            enum_name: "status".to_string(),
            old_values: vec!["active".to_string(), "inactive".to_string()],
            new_values: vec!["inactive".to_string(), "active".to_string()],
            added_values: Vec::new(),
            removed_values: Vec::new(),
            change_kind: EnumChangeKind::Recreate,
            columns: vec![EnumColumnRef {
                table_name: "users".to_string(),
                column_name: "status".to_string(),
            }],
        });

        let pipeline =
            MigrationPipeline::new(&diff, Dialect::PostgreSQL).with_allow_destructive(true);
        let result = pipeline.generate_up();

        assert!(result.is_ok());
        let (sql, _) = result.unwrap();
        assert!(sql.contains(r#"ALTER TYPE "status" RENAME TO "status_old""#));
        assert!(sql.contains(r#"CREATE TYPE "status" AS ENUM ('inactive', 'active')"#));
        assert!(sql.contains(r#"DROP TYPE "status_old""#));
    }
}
