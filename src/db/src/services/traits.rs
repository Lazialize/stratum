// サービストレイト定義
//
// テスト時のモック差し替えを可能にするためのトレイト群。
// 各サービスの公開インターフェースを抽象化します。

use crate::core::config::Dialect;
use crate::core::destructive_change_report::DestructiveChangeReport;
use crate::core::error::{ValidationResult, ValidationWarning};
use crate::core::schema::Schema;
use crate::core::schema_diff::SchemaDiff;

/// スキーマ差分検出サービスのトレイト
pub trait SchemaDiffDetector {
    /// スキーマ差分を検出（警告付き）
    fn detect_diff_with_warnings(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> (SchemaDiff, Vec<ValidationWarning>);
}

/// スキーマバリデーションサービスのトレイト
pub trait SchemaValidator {
    /// カラムリネームの検証（旧スキーマ照合あり）
    fn validate_renames_with_old_schema(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> ValidationResult;
}

/// マイグレーション生成サービスのトレイト
pub trait MigrationGenerator {
    /// タイムスタンプを生成
    fn generate_timestamp(&self) -> String;

    /// 説明文をファイル名用にサニタイズ
    fn sanitize_description(&self, description: &str) -> String;

    /// マイグレーションファイル名を生成
    fn generate_migration_filename(&self, timestamp: &str, description: &str) -> String;

    /// UP SQLを生成（スキーマ付き、破壊的変更許可付き）
    fn generate_up_sql_with_schemas_and_options(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<(String, ValidationResult), String>;

    /// DOWN SQLを生成（スキーマ付き、破壊的変更許可付き）
    fn generate_down_sql_with_schemas_and_options(
        &self,
        diff: &SchemaDiff,
        old_schema: &Schema,
        new_schema: &Schema,
        dialect: Dialect,
        allow_destructive: bool,
    ) -> Result<(String, ValidationResult), String>;

    /// マイグレーションメタデータを生成
    fn generate_migration_metadata(
        &self,
        version: &str,
        description: &str,
        dialect: Dialect,
        checksum: &str,
        destructive_changes: DestructiveChangeReport,
    ) -> Result<String, String>;
}
