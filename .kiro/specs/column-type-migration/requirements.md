# Requirements Document

## Introduction
本機能は、Stratumにおいて既存テーブルのカラム型変更を検出し、対応するマイグレーションSQL（up/down）を自動生成する機能を実装します。これにより、スキーマ定義の変更から安全かつ一貫性のあるカラム型変更マイグレーションを生成できるようになります。

## Requirements

### Requirement 1: カラム型変更の検出
**Objective:** As a バックエンドエンジニア, I want スキーマ定義ファイルでカラムの型を変更したときに差分として検出される, so that 型変更を含むマイグレーションを自動生成できる

#### Acceptance Criteria
1. When YAMLスキーマファイルで既存カラムの型が変更された場合, the SchemaDiffDetector shall カラム型変更を差分として検出する
2. When カラム型が変更された場合, the SchemaDiffDetector shall 変更前の型と変更後の型の両方を差分情報に含める
3. The SchemaDiffDetector shall 同一テーブル内の複数カラムの型変更を同時に検出できる
4. The SchemaDiffDetector shall 型変更と他の変更（カラム追加・削除・制約変更）を区別して検出する

### Requirement 2: 型変更マイグレーションSQLの生成
**Objective:** As a バックエンドエンジニア, I want カラム型変更に対応するALTER TABLE文が自動生成される, so that 手動でSQLを書く必要がなくなる

#### Acceptance Criteria
1. When カラム型変更が検出された場合, the SqlGenerator shall 適切なALTER TABLE文（up.sql）を生成する
2. When カラム型変更が検出された場合, the SqlGenerator shall ロールバック用のALTER TABLE文（down.sql）を生成する
3. When PostgreSQLが対象の場合, the PostgresGenerator shall `ALTER TABLE ... ALTER COLUMN ... TYPE ...` 構文を使用する
4. When MySQLが対象の場合, the MySqlGenerator shall `ALTER TABLE ... MODIFY COLUMN ...` 構文を使用する
5. When SQLiteが対象の場合, the SqliteGenerator shall テーブル再作成を使用した型変更を実装する（SQLiteはALTER COLUMN TYPEをサポートしないため）

### Requirement 3: 方言間の型マッピング
**Objective:** As a バックエンドエンジニア, I want 異なるデータベース方言でも一貫した型変更が行える, so that マルチデータベース環境でも安全に運用できる

#### Acceptance Criteria
1. The SqlGenerator shall INTEGER, VARCHAR, TEXT, BOOLEAN, DECIMAL, DATE, TIMESTAMP, UUID, JSONB等の標準型間の変更をサポートする
2. When 型変更が方言固有の制約に違反する場合, the SqlGenerator shall 適切なエラーメッセージを返す
3. The SqlGenerator shall 各方言で同等の型変更が行われるようマッピングを適用する

### Requirement 4: 型変更の検証
**Objective:** As a DevOps/SREチーム, I want 危険な型変更に対して警告が表示される, so that データ損失のリスクを事前に把握できる

#### Acceptance Criteria
1. When データ損失の可能性がある型変更（例: VARCHAR→INTEGER、TEXT→BOOLEAN）が検出された場合, the SchemaValidator shall 警告メッセージを出力する
2. When 精度が低下する可能性がある型変更（例: DECIMAL(10,2)→DECIMAL(5,2)）が検出された場合, the SchemaValidator shall 警告メッセージを出力する
3. If 型変更が明らかに不正な場合（例: 互換性のない型間の変換）, then the SchemaValidator shall エラーを返しマイグレーション生成を中止する

### Requirement 5: dry-runモードでの型変更プレビュー
**Objective:** As a バックエンドエンジニア, I want 型変更マイグレーションを適用前にプレビューできる, so that 意図した変更かどうか確認できる

#### Acceptance Criteria
1. When dry-runモードで型変更を含むマイグレーションを生成した場合, the CLI shall 生成されるSQLをコンソールに表示する
2. When dry-runモードの場合, the CLI shall 実際のマイグレーションファイルを作成しない
3. The CLI shall 型変更の前後の型情報を人間が読みやすい形式で表示する
