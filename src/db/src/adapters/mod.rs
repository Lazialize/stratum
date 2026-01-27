// Adapters
// データベースとファイルシステムへのアクセスを抽象化

pub mod connection_string;
pub mod database;
pub mod database_introspector;
pub mod database_migrator;
pub mod sql_generator;
pub mod sql_quote;
pub mod type_mapping;
