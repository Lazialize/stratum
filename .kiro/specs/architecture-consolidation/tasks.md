# Implementation Plan

## Phase 1: 基盤整備（TypeMapping, DtoConverter）

- [x] 1. TypeMappingService の作成と型変換ロジックの集約
- [x] 1.1 (P) TypeMapper トレイトと方言別実装の作成
  - 型変換の共通インターフェースを定義し、PostgreSQL/MySQL/SQLite の各方言に対応した実装を提供する
  - ColumnType から SQL 型文字列への変換（to_sql_type）を実装する
  - SQL 型文字列から ColumnType への逆変換（from_sql_type）を実装し、メタデータ（precision, scale, length）を適切に処理する
  - 方言固有の型（PostgreSQL の SERIAL、MySQL の TINYINT(1) など）をフック経由で処理する
  - デフォルト型（TEXT）へのフォールバック機構を実装する
  - 各方言の型変換が正しく動作することを単体テストで検証する
  - _Requirements: 1.1, 1.2, 1.4_

- [x] 1.2 既存コードから TypeMappingService への移行
  - ColumnType::to_sql_type を TypeMappingService への委譲に変更し、後方互換性を維持する
  - 各 SqlGenerator の map_column_type を TypeMappingService 呼び出しに置き換える
  - export.rs の parse_sqlite_type, parse_postgres_type, parse_mysql_type を TypeMappingService::from_sql_type に移行する
  - 重複していた型変換ロジックを削除し、単一の実装に統合する
  - 既存の全テストがパスすることを確認する
  - _Requirements: 1.3, 1.5_

- [x] 2. DtoConverterService の作成とラウンドトリップ整合性の保証
- [x] 2.1 (P) DtoConverterService の基本実装
  - Schema ↔ SchemaDto の双方向変換を単一サービスに集約する
  - Table ↔ TableDto の変換で PRIMARY_KEY 制約を primary_key フィールドとして適切に処理する
  - Constraint ↔ ConstraintDto の変換で各制約タイプ（FOREIGN_KEY, UNIQUE, CHECK）を正しく処理する
  - Index の変換でカラムリストと unique フラグを保持する
  - 変換エラー時に明確なエラーメッセージを返す
  - _Requirements: 6.1, 6.2_

- [x] 2.2 ラウンドトリップテストの実装と既存コードの移行
  - Schema を YAML にシリアライズし再パースした場合に元のオブジェクトと同一になることを検証するテストを追加する
  - schema_parser.rs の変換ロジックを DtoConverterService 呼び出しに移行する
  - schema_serializer.rs の変換ロジックを DtoConverterService 呼び出しに移行する
  - 既存のラウンドトリップテストが引き続きパスすることを確認する
  - 新しいフィールド追加時に単一箇所の変更で対応できることを検証する
  - _Requirements: 6.3, 6.4, 6.5_

## Phase 2: export 責務分離

- [x] 3. DatabaseIntrospector の作成と introspection ロジックの分離
- [x] 3.1 (P) DatabaseIntrospector トレイトと方言別実装の作成
  - データベースからテーブル名一覧を取得する機能を実装する
  - テーブルごとのカラム情報（名前、型、NULL可否、デフォルト値）を取得する機能を実装する
  - インデックス情報（名前、カラム、ユニーク性）を取得する機能を実装する
  - 制約情報（PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK）を取得する機能を実装する
  - PostgreSQL 専用の ENUM 定義取得機能を実装する
  - 各方言の INFORMATION_SCHEMA / PRAGMA クエリを正しく実行できることをテストで検証する
  - _Requirements: 3.1, 3.3_

- [x] 3.2 (P) SchemaConversionService の作成
  - DatabaseIntrospector から取得した生データを内部モデル（Column, Index, Constraint）に変換する
  - TypeMappingService を使用して SQL 型文字列を ColumnType に変換する
  - 複数テーブルの情報を Schema オブジェクトに組み立てる
  - 変換エラー時に発生箇所を明確にしたエラーメッセージを返す
  - 変換処理を個別にユニットテスト可能な設計とする
  - _Requirements: 3.4, 3.5_

- [x] 3.3 export コマンドの責務分離と統合
  - export.rs から introspection ロジックを DatabaseIntrospector に移行する
  - export.rs から型変換ロジックを SchemaConversionService に移行する
  - export.rs は出力処理（YAML シリアライズ、ファイル/標準出力）のみに責務を限定する
  - 依存関係が adapters → services の方向に正しく流れることを確認する
  - 既存の export コマンドの動作が変わらないことを統合テストで検証する
  - _Requirements: 3.2, 7.1, 7.2_

## Phase 3: マイグレーションパイプライン統合

- [x] 4. MigrationPipeline の実装と分岐ロジックの統合
- [x] 4.1 MigrationPipeline 構造体とステージ処理の実装
  - 共通パイプライン構造体を作成し、SchemaDiff と Dialect を受け取る
  - with_schemas メソッドでスキーマ情報を設定し、型変更検証を有効化する
  - prepare ステージで SqlGenerator を取得し事前検証を行う
  - enum_statements ステージで ENUM 作成/変更（PostgreSQL）を処理する
  - table_statements ステージで CREATE/ALTER TABLE を生成する
  - index_statements ステージで CREATE INDEX を生成する
  - constraint_statements ステージで制約追加を処理する
  - cleanup_statements ステージで DROP TABLE/TYPE を処理する
  - finalize ステージで SQL 文を結合する
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 4.2 既存 API の後方互換性維持とテスト
  - generate_up_sql を MigrationPipeline のラッパーとして実装する
  - generate_up_sql_with_schemas を MigrationPipeline のラッパーとして実装する
  - パイプラインの特定ステージでエラーが発生した場合に発生箇所を明確にしたエラーメッセージを返す
  - ゴールデンテストを追加し、既存のマイグレーション SQL 出力が変更されないことを検証する
  - migration_generator.rs の重複していた分岐ロジックを削除する
  - _Requirements: 2.4, 2.5, 7.3_

## Phase 4: バリデーション分割 & SQL安全化

- [x] 5. SchemaValidatorService のバリデーションロジック分割
- [x] 5.1 (P) カテゴリ別バリデーション関数の抽出
  - validate_enums 関数を抽出し、ENUM 定義の検証（重複チェック、PostgreSQL 固有チェック）を実装する
  - validate_column_types 関数を抽出し、カラム型の検証（DECIMAL 範囲、CHAR 長さ等）を実装する
  - validate_primary_keys 関数を抽出し、プライマリキーの存在確認を実装する
  - validate_index_references 関数を抽出し、インデックスのカラム参照整合性を検証する
  - validate_constraint_references 関数を抽出し、制約のカラム/テーブル参照整合性を検証する
  - 各関数が50行を超えないように構成する
  - _Requirements: 4.1, 4.2, 4.4_

- [x] 5.2 バリデーション結果の統合とエラー/警告分離
  - 各カテゴリの検証結果を ValidationResult にマージする統合ロジックを実装する
  - 複数のバリデーションエラーが発生した場合に全てのエラーを収集して一括報告する
  - エラーと警告を明確に区別して返す
  - 既存の validate メソッドを統合エントリポイントとして維持し、後方互換性を保つ
  - 各検証カテゴリを個別にユニットテスト可能な設計とする
  - _Requirements: 4.3, 4.5, 7.4_

- [x] 6. DatabaseMigrator の SQL 組み立て安全化
- [x] 6.1 (P) テーブル名の許可リスト検証の実装
  - マイグレーションテーブル名のバリデーション関数を追加する（英字/アンダースコア開始、英数字とアンダースコアのみ、最大63文字）
  - DatabaseMigrator のコンストラクタでテーブル名を検証し、不正な名前を拒否する
  - バリデーション失敗時に明確なエラーメッセージを返す
  - デフォルトのマイグレーションテーブル名（schema_migrations）を定数として定義する
  - _Requirements: 5.3_

- [x] 6.2 パラメータバインディングへの移行
  - generate_record_migration_sql を generate_record_migration_query に変更し、SQL 文字列とバインドパラメータを分離して返す
  - generate_remove_migration_sql を同様にバインド方式に変更する
  - record_migration メソッドで sqlx::query().bind() を使用してパラメータをバインドする
  - 各方言（PostgreSQL: $1, MySQL/SQLite: ?）のプレースホルダを正しく使用する
  - 文字列補間による SQL 組み立てがコードレビューで検出可能な形式に制限されていることを確認する
  - _Requirements: 5.1, 5.2, 5.5_

- [x] 6.3* CI 検証戦略のセットアップ
  - PostgreSQL 用の CI ジョブを追加し、cargo sqlx prepare --check を実行する
  - MySQL 用の CI ジョブを追加し、cargo sqlx prepare --check を実行する
  - SQLite 用の CI ジョブを追加し、cargo sqlx prepare --check を実行する
  - sqlx のコンパイル時クエリ検証機能を活用する
  - _Requirements: 5.4_

## Phase 5: 統合テストと品質保証

- [x] 7. 全体統合と品質基準の確認
- [x] 7.1 全テストスイートの実行と回帰確認
  - cargo test で全 152 以上のユニットテストがパスすることを確認する
  - 27 以上のテストスイートが全てパスすることを確認する
  - 統合テストでエンドツーエンドの動作を検証する
  - _Requirements: 7.4, 8.5_

- [x] 7.2 コード品質基準の確認
  - cargo fmt で全コードが自動フォーマットに準拠することを確認する
  - cargo clippy で警告が 0 件であることを確認する
  - 不要な .clone() が追加されていないことをレビューする
  - unwrap()/expect() が本番コードで使用されていないことを確認する
  - 変更した公開 API にドキュメントコメントが付与されていることを確認する
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [x] 7.3 後方互換性の最終検証
  - 既存の公開 API シグネチャが変更されていないことを確認する
  - 既存の YAML スキーマフォーマットとの互換性を検証する
  - 生成されるマイグレーション SQL の出力形式が変更されていないことを検証する
  - _Requirements: 7.1, 7.2, 7.3_
