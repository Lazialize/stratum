// Core Domain
// スキーマ解析、差分検出、検証、マイグレーション生成の純粋なビジネスロジック

pub mod config;
pub mod destructive_change_report;
pub mod error;
pub mod migration;
pub mod naming;
pub mod schema;
pub mod schema_diff;
pub mod type_category;
