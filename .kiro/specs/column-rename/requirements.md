# Requirements Document

## Introduction
本仕様は、Stratumにおけるカラムリネーム機能の追加に関する要件を定義します。現在、スキーマの差分検出ではカラムの追加・削除は検出できますが、カラム名の変更（リネーム）を検出し、適切なマイグレーションSQLを生成する機能が不足しています。この機能により、ユーザーはカラム名を変更した際にデータを保持したまま安全にマイグレーションを実行できるようになります。

## Requirements

### Requirement 1: スキーマ定義でのカラムリネーム指定
**Objective:** As a バックエンドエンジニア, I want YAMLスキーマ定義でカラムのリネームを明示的に指定できること, so that 意図したカラム名変更がマイグレーションに正しく反映される

#### Acceptance Criteria
1. When ユーザーがカラム定義に`renamed_from`属性を指定した場合, the Schema Parser shall 旧カラム名と新カラム名のマッピング情報を保持する
2. When `renamed_from`属性が指定されたカラムをパースした場合, the Schema Parser shall カラムオブジェクトにリネーム元の情報を設定する
3. The Schema Parser shall `renamed_from`属性を持つカラム定義を正しくYAMLからデシリアライズする

### Requirement 2: カラムリネームの差分検出
**Objective:** As a バックエンドエンジニア, I want スキーマの差分検出でカラムのリネームを正しく識別できること, so that 不要なカラム削除・追加ではなくリネーム操作としてマイグレーションが生成される

#### Acceptance Criteria
1. When 新スキーマのカラムに`renamed_from`属性が指定されている場合, the Schema Diff Detector shall 該当する変更をカラムリネームとして検出する
2. When カラムリネームが検出された場合, the Schema Diff Detector shall リネーム情報を`SchemaDiff`に含める
3. The Schema Diff Detector shall カラムリネームと同時に行われた型変更やNULL制約変更も検出する
4. If 同一テーブル内で複数のカラムリネームが存在する場合, the Schema Diff Detector shall 全てのリネームを正しく検出する

### Requirement 3: リネーム用SQLの生成
**Objective:** As a バックエンドエンジニア, I want 各データベース方言に対応したカラムリネームSQLが生成されること, so that PostgreSQL、MySQL、SQLiteそれぞれで正しくマイグレーションを実行できる

#### Acceptance Criteria
1. When カラムリネームの差分が検出された場合, the PostgreSQL Generator shall `ALTER TABLE ... RENAME COLUMN ... TO ...`形式のSQLを生成する
2. When カラムリネームの差分が検出された場合, the MySQL Generator shall `ALTER TABLE ... CHANGE COLUMN ...`形式のSQLを生成する
3. When カラムリネームの差分が検出された場合, the SQLite Generator shall `ALTER TABLE ... RENAME COLUMN ... TO ...`形式のSQLを生成する
4. The SQL Generator shall リネームのdown.sql（ロールバック用）として逆方向のリネームSQLを生成する
5. When カラムリネームと同時に型変更が行われた場合, the SQL Generator shall リネームと型変更を適切な順序で実行するSQLを生成する

### Requirement 4: カラムリネームの検証
**Objective:** As a バックエンドエンジニア, I want カラムリネーム操作の妥当性が検証されること, so that 無効なリネーム操作によるマイグレーション失敗を防止できる

#### Acceptance Criteria
1. If `renamed_from`で指定されたカラム名が旧スキーマに存在しない場合, the Schema Validator shall 警告メッセージを表示し、該当の`renamed_from`属性を無視する
2. If 同じ旧カラム名が複数のカラムで`renamed_from`に指定されている場合, the Schema Validator shall 重複エラーを返す
3. If リネーム先のカラム名が既存のカラム名と衝突する場合, the Schema Validator shall 名前衝突エラーを返す
4. The Schema Validator shall リネームされるカラムが外部キー制約で参照されている場合に警告を出す
5. When マイグレーション生成後に`renamed_from`属性が残っている場合, the CLI shall 属性の削除を推奨する警告を表示する

### Requirement 5: エラーハンドリングとユーザーフィードバック
**Objective:** As a バックエンドエンジニア, I want カラムリネームに関するエラーが明確なメッセージで報告されること, so that 問題の原因を迅速に特定し修正できる

#### Acceptance Criteria
1. If カラムリネームの検証に失敗した場合, the CLI shall 旧カラム名、新カラム名、テーブル名を含むエラーメッセージを表示する
2. When dry-runモードでカラムリネームを含むマイグレーションを実行した場合, the CLI shall 生成されるリネームSQLをプレビュー表示する
3. If データベース側でリネーム操作が失敗した場合, the Database Migrator shall 具体的なエラー原因を含むメッセージを返す
