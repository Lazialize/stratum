// Stratumライブラリのエントリーポイント
//
// モジュール構造:
// - cli: CLIレイヤー（ユーザー入力の受付とコマンドルーティング）
// - core: コアドメインロジック（スキーマ解析、差分検出、検証、マイグレーション生成）
// - adapters: データベースとファイルシステムへのアクセスを抽象化

pub mod cli;
pub mod core;
pub mod adapters;
pub mod services;
