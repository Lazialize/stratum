# Implementation Tasks: schema-code-management-cli

## 1. プロジェクトセットアップとCLI基盤の実装
Rustプロジェクトの初期化とCLI基盤の構築

- [ ] 1. プロジェクトの初期化と依存関係の設定 (P)
  - Cargoプロジェクトの作成
  - 必要なクレート（clap, tokio, sqlx, serde-saphyr, anyhow, thiserror, sha2）の追加
  - ワークスペース構成の設定（必要に応じて）
  - _Requirements: 10_

- [ ] 2. CLIエントリーポイントの実装 (P)
  - clapのderive APIを使用したCLI構造の定義
  - サブコマンド（init, generate, apply, rollback, validate, status, export）の宣言
  - グローバルオプション（--config, --verbose）の定義
  - _Requirements: 10_

- [ ] 3. 設定ファイル管理機能の実装 (P)
  - Config構造体の定義（dialect, connection settings）
  - 設定ファイルの読み込み機能（YAML形式）
  - 設定ファイルのバリデーション
  - _Requirements: 2, 8_

## 2. コア型定義とドメインモデルの実装
スキーマ定義を表現する型システムの構築

- [ ] 4. スキーマドメインモデルの実装 (P)
  - Schema, Table, Column, Index, Constraint, ForeignKey構造体の定義
  - serde Deserialize/Serializeトレイトの実装
  - Dialectの型定義（PostgreSQL, MySQL, SQLite）
  - _Requirements: 1_

- [ ] 5. Migrationドメインモデルの実装 (P)
  - Migration構造体の定義（version, timestamp, checksum）
  - MigrationStatus列挙型の定義
  - マイグレーション履歴を表す型の定義
  - _Requirements: 3, 4, 5, 7_

- [ ] 6. エラー型の定義と実装 (P)
  - thiserrorを使用したカスタムエラー型の定義
  - エラーカテゴリ（ValidationError, DatabaseError, IoError）の実装
  - エラーメッセージの日本語対応
  - _Requirements: 1, 3, 4, 5, 6, 7, 8, 9_

## 3. YAMLスキーマ解析機能の実装
スキーマ定義ファイルの読み込みと検証

- [ ] 7. SchemaParserServiceの実装
  - スキーマディレクトリのスキャン機能
  - YAML解析（serde-saphyr）の実装
  - スキーマファイルのマージ処理
  - _Requirements: 1_

- [ ] 8. スキーマバリデーション機能の実装
  - テーブル定義の検証（必須フィールド、型チェック）
  - インデックス定義の検証
  - 外部キー制約の検証（参照先テーブルの存在確認）
  - _Requirements: 6_

- [ ] 9. スキーマチェックサム計算機能の実装 (P)
  - SHA-256ハッシュ計算の実装
  - 正規化されたスキーマ表現の生成
  - チェックサムの比較機能
  - _Requirements: 7_

## 4. データベース接続管理の実装
SQLxを使用した統一データベースアクセス層

- [ ] 10. DatabaseConnectionServiceの実装
  - SQLxプールの初期化（PostgreSQL/MySQL/SQLite）
  - 接続テスト機能の実装
  - 接続エラーハンドリング
  - _Requirements: 8_

- [ ] 11. DatabaseMigratorServiceの実装
  - マイグレーション履歴テーブルの作成（schema_migrations）
  - トランザクション制御の実装
  - データベース固有のSQL構文の抽象化
  - _Requirements: 4, 5, 8_

## 5. SQL生成機能の実装
スキーマ定義からDDL文を生成

- [ ] 12. PostgreSQL用SQLジェネレーターの実装
  - CREATE TABLE文の生成
  - CREATE INDEX文の生成
  - ALTER TABLE（制約追加）の生成
  - _Requirements: 3, 8_

- [ ] 13. MySQL用SQLジェネレーターの実装
  - CREATE TABLE文の生成（MySQLの型マッピング）
  - CREATE INDEX文の生成
  - ALTER TABLE（制約追加）の生成
  - _Requirements: 3, 8_

- [ ] 14. SQLite用SQLジェネレーターの実装
  - CREATE TABLE文の生成（SQLiteの型マッピング）
  - CREATE INDEX文の生成
  - ALTER TABLEの制限事項への対応
  - _Requirements: 3, 8_

## 6. マイグレーションファイル生成機能の実装
スキーマ差分からマイグレーションを生成

- [ ] 15. スキーマ差分検出機能の実装
  - 現在のスキーマと前回のスキーマの比較
  - テーブル追加/削除/変更の検出
  - カラム追加/削除/変更の検出
  - インデックスと制約の差分検出
  - _Requirements: 3_

- [ ] 16. マイグレーションファイル生成機能の実装
  - タイムスタンプベースのファイル名生成
  - up.sqlとdown.sqlの生成
  - マイグレーションメタデータファイルの生成（.meta.yaml）
  - _Requirements: 3_

## 7. initコマンドの実装
プロジェクトの初期化機能

- [ ] 17. initコマンドハンドラーの実装
  - プロジェクトディレクトリ構造の作成（schema/, migrations/）
  - デフォルト設定ファイルの生成（.schema-manager.yaml）
  - ダイアレクトと接続設定の対話的入力
  - 初期化済みプロジェクトの検出と警告
  - _Requirements: 2_

## 8. generateコマンドの実装
マイグレーションファイル生成機能

- [ ] 18. generateコマンドハンドラーの実装
  - スキーマ定義の読み込み
  - 前回のスキーマ状態の読み込み
  - 差分検出とマイグレーションファイル生成
  - 生成されたファイルパスの表示
  - _Requirements: 3_

## 9. applyコマンドの実装
マイグレーションの適用機能

- [ ] 19. applyコマンドハンドラーの実装
  - データベース接続の確立
  - 未適用マイグレーションの検出
  - マイグレーションの順次実行（トランザクション内）
  - 実行結果の記録とチェックサムの保存
  - 実行ログの表示
  - _Requirements: 4_

## 10. rollbackコマンドの実装
マイグレーションのロールバック機能

- [ ] 20. rollbackコマンドハンドラーの実装
  - 最新の適用済みマイグレーションの特定
  - down.sqlの実行（トランザクション内）
  - マイグレーション履歴からの削除
  - ロールバック結果の表示
  - _Requirements: 5_

## 11. validateコマンドの実装
スキーマ検証機能

- [ ] 21. validateコマンドハンドラーの実装
  - スキーマ定義ファイルの読み込み
  - バリデーションルールの実行
  - エラーと警告のフォーマットされた表示
  - 検証結果のサマリー表示
  - _Requirements: 6_

## 12. statusコマンドの実装
マイグレーション状態の確認機能

- [ ] 22. statusコマンドハンドラーの実装
  - データベース接続と履歴テーブルの読み込み
  - ローカルマイグレーションファイルとの照合
  - 適用済み/未適用の状態表示（テーブル形式）
  - チェックサム不一致の検出と警告
  - _Requirements: 7_

## 13. exportコマンドの実装
スキーマのエクスポート機能

- [ ] 23. exportコマンドハンドラーの実装
  - データベースからのスキーマ情報取得（INFORMATION_SCHEMA）
  - スキーマ定義のYAML形式への変換
  - ファイルへの出力または標準出力への表示
  - ダイアレクト固有の型の正規化
  - _Requirements: 9_

## 14. ヘルプとドキュメント
ユーザー向けドキュメントとヘルプの整備

- [ ] 24. CLIヘルプテキストの充実化 (P)
  - 各サブコマンドの詳細ヘルプの記述
  - 使用例の追加
  - エラーメッセージの改善
  - _Requirements: 10_

- [ ] 25. README.mdの作成 (P)
  - インストール手順の記述
  - クイックスタートガイドの作成
  - 各コマンドの使用例の記載
  - 設定ファイルの説明
  - _Requirements: 10_

## 15. テストの実装
ユニットテストと統合テストの作成

- [ ] 26. ドメインモデルのユニットテストの実装 (P)
  - Schema, Table, Column等の構造体のテスト
  - YAMLシリアライゼーション/デシリアライゼーションのテスト
  - バリデーション機能のテスト
  - _Requirements: 1, 6_

- [ ] 27. SQL生成機能のユニットテストの実装 (P)
  - 各データベースのSQLジェネレーターのテスト
  - エッジケースの検証（NULL許可、デフォルト値など）
  - スキーマ差分検出のテスト
  - _Requirements: 3_

- [ ] 28. データベース統合テストの実装
  - testcontainersを使用したテスト環境のセットアップ
  - マイグレーション適用/ロールバックのエンドツーエンドテスト
  - チェックサム検証のテスト
  - _Requirements: 4, 5, 7_

## 16. ビルドとリリース準備

- [ ] 29. マルチプラットフォームビルド設定 (P)
  - cargo buildの最適化設定（release profile）
  - クロスコンパイル設定（Linux, macOS, Windows）
  - バイナリサイズの最適化
  - _Requirements: 10_

- [ ] 30. リリースドキュメントの作成 (P)
  - CHANGELOG.mdの作成
  - LICENSEファイルの配置
  - コントリビューションガイドラインの作成（必要に応じて）
  - _Requirements: 10_
