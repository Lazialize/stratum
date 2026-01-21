# Requirements Document

## Project Description (Input)
データベースのスキーマ定義をコードで管理するCLIツール

## Introduction
本仕様書は、データベーススキーマをコードとして定義・管理するCLIツールの要件を定義します。このツールは、スキーマのバージョン管理、マイグレーション実行、スキーマ定義の検証を提供し、Infrastructure as Code (IaC) の原則に基づいたデータベース管理を実現します。

## Requirements

### Requirement 1: スキーマ定義の管理
**Objective:** As a データベース管理者, I want スキーマ定義をコードファイルとして作成・編集する, so that バージョン管理システムでスキーマの変更履歴を追跡できる

#### Acceptance Criteria
1. The CLI shall スキーマ定義ファイルを指定されたディレクトリから読み込む
2. The CLI shall テーブル、カラム、インデックス、制約の定義を解析する
3. When スキーマ定義ファイルに構文エラーが存在する, the CLI shall エラー内容と行番号を含む詳細なエラーメッセージを表示する
4. The CLI shall 複数のスキーマ定義ファイルを統合して単一のスキーマ構造を生成する
5. The CLI shall スキーマ定義のフォーマットとして YAML を使用する

### Requirement 2: スキーマの初期化
**Objective:** As a 開発者, I want 新規プロジェクトのスキーマ管理を簡単に開始する, so that 初期セットアップの時間を削減できる

#### Acceptance Criteria
1. When ユーザーが init コマンドを実行する, the CLI shall プロジェクトのルートディレクトリにスキーマ管理用の設定ファイルを作成する
2. The CLI shall 設定ファイルに dialect (postgresql/mysql/sqlite) とデータベース接続設定 (host, port, database, user, password) を含める
3. The CLI shall スキーマ定義ファイルを配置するためのデフォルトディレクトリ構造を生成する
4. The CLI shall サンプルスキーマ定義ファイルをテンプレートとして提供する
5. If 既に設定ファイルが存在する, then the CLI shall 上書き確認のプロンプトを表示する

### Requirement 3: マイグレーションファイルの生成
**Objective:** As a 開発者, I want スキーマの変更を自動的にマイグレーションファイルに変換する, so that データベースへの適用を安全に管理できる

#### Acceptance Criteria
1. When ユーザーが migration generate コマンドを実行する, the CLI shall 現在のスキーマ定義と前回のスキーマ定義を比較する
2. The CLI shall 差分を検出し、CREATE、ALTER、DROP 文を含むマイグレーションファイルを生成する
3. The CLI shall マイグレーションファイルにタイムスタンプベースの一意な識別子を付与する
4. The CLI shall マイグレーションファイルに up (適用) と down (ロールバック) の両方のスクリプトを含める
5. If スキーマに変更が検出されない, then the CLI shall マイグレーションファイルを生成せず、変更なしのメッセージを表示する

### Requirement 4: マイグレーションの実行
**Objective:** As a データベース管理者, I want マイグレーションを安全にデータベースに適用する, so that スキーマ変更を制御された方法で反映できる

#### Acceptance Criteria
1. When ユーザーが migration apply コマンドを実行する, the CLI shall 未適用のマイグレーションファイルを検出する
2. The CLI shall マイグレーションをタイムスタンプ順に実行する
3. While マイグレーション実行中, the CLI shall 進行状況とログを標準出力に表示する
4. If マイグレーション実行中にエラーが発生する, then the CLI shall トランザクションをロールバックし、エラー詳細を表示する
5. The CLI shall 適用済みマイグレーションの履歴を管理テーブルに記録する
6. When --dry-run フラグが指定される, the CLI shall 実行されるSQL文をプレビュー表示し、データベースには適用しない

### Requirement 5: マイグレーションのロールバック
**Objective:** As a データベース管理者, I want 適用済みマイグレーションを取り消す, so that 問題発生時に以前の状態に戻すことができる

#### Acceptance Criteria
1. When ユーザーが migration rollback コマンドを実行する, the CLI shall 最新の適用済みマイグレーションを特定する
2. The CLI shall 該当マイグレーションの down スクリプトを実行する
3. The CLI shall ロールバック完了後、マイグレーション履歴テーブルから該当レコードを削除する
4. If ロールバック中にエラーが発生する, then the CLI shall エラーを表示し、マイグレーション履歴を変更しない
5. When --steps オプションが指定される, the CLI shall 指定された数のマイグレーションをロールバックする

### Requirement 6: スキーマの検証
**Objective:** As a 開発者, I want スキーマ定義の整合性を検証する, so that デプロイ前に問題を検出できる

#### Acceptance Criteria
1. When ユーザーが validate コマンドを実行する, the CLI shall スキーマ定義ファイルの構文をチェックする
2. The CLI shall 外部キー制約の参照整合性を検証する
3. The CLI shall テーブル名、カラム名の命名規則違反を検出する
4. The CLI shall 重複したインデックス定義を警告する
5. If 検証エラーが存在する, then the CLI shall エラー一覧と推奨される修正方法を表示する
6. If すべての検証に合格する, then the CLI shall 成功メッセージと検証統計を表示する

### Requirement 7: スキーマ状態の確認
**Objective:** As a 開発者, I want 現在のスキーマ状態とマイグレーション状況を確認する, so that データベースの現状を把握できる

#### Acceptance Criteria
1. When ユーザーが status コマンドを実行する, the CLI shall 適用済みマイグレーションの一覧を表示する
2. The CLI shall 未適用のマイグレーションファイルを検出し、リストアップする
3. The CLI shall データベースの現在のスキーマバージョンを表示する
4. The CLI shall スキーマ定義ファイルとデータベースの実際のスキーマの差分を検出する
5. If スキーマ定義とデータベースが同期していない, then the CLI shall 警告メッセージと推奨アクションを表示する

### Requirement 8: データベース接続管理
**Objective:** As a 開発者, I want 複数のデータベース環境に接続する, so that 開発・ステージング・本番環境を管理できる

#### Acceptance Criteria
1. The CLI shall 環境別の接続情報を設定ファイルから読み込む
2. The CLI shall 環境変数からデータベース接続情報を取得する
3. When --env オプションが指定される, the CLI shall 指定された環境の接続情報を使用する
4. If 接続情報が不足している, then the CLI shall 不足しているパラメータを明示したエラーメッセージを表示する
5. The CLI shall PostgreSQL、MySQL、SQLite の接続をサポートする
6. While データベース接続を確立する, the CLI shall 接続タイムアウトを設定可能にする

### Requirement 9: スキーマのエクスポート
**Objective:** As a データベース管理者, I want 既存データベースのスキーマをコード定義としてエクスポートする, so that 既存システムへの導入を容易にする

#### Acceptance Criteria
1. When ユーザーが export コマンドを実行する, the CLI shall データベースの現在のスキーマ構造を読み取る
2. The CLI shall テーブル、カラム、インデックス、制約をスキーマ定義ファイル形式に変換する
3. The CLI shall エクスポート先のディレクトリとファイル形式を指定可能にする
4. If エクスポート先に既存ファイルが存在する, then the CLI shall 上書き確認または自動バックアップを実行する
5. The CLI shall エクスポート完了後、生成されたファイルのパスと統計情報を表示する

### Requirement 10: CLI のヘルプとドキュメント
**Objective:** As a ユーザー, I want コマンドの使い方を簡単に確認する, so that 学習コストを削減できる

#### Acceptance Criteria
1. When ユーザーが --help フラグを指定する, the CLI shall 利用可能なコマンド一覧と概要を表示する
2. When ユーザーが [command] --help を実行する, the CLI shall 該当コマンドの詳細な使用方法、オプション、例を表示する
3. The CLI shall エラーメッセージに関連するヘルプコマンドの提案を含める
4. The CLI shall --version フラグでツールのバージョン情報を表示する
5. The CLI shall カラー出力をサポートし、--no-color オプションで無効化可能にする

## Non-Functional Requirements

### Performance
- The CLI shall 1000テーブル規模のスキーマ定義を10秒以内に解析する
- The CLI shall マイグレーション生成処理を5秒以内に完了する

### Reliability
- The CLI shall マイグレーション適用時にトランザクションを使用し、失敗時の自動ロールバックを保証する
- The CLI shall 不正な入力に対してグレースフルに失敗し、詳細なエラー情報を提供する

### Usability
- The CLI shall 一貫したコマンド構文とオプション命名規則を使用する
- The CLI shall 進行状況インジケーターとカラー出力で視覚的なフィードバックを提供する

### Compatibility
- The CLI shall Node.js 18 以降の環境で動作する
- The CLI shall Linux、macOS、Windows で動作する

### Database Support
- The CLI shall PostgreSQL、MySQL、SQLite の3つのデータベースシステムをサポートする
- The CLI shall dialect 設定に基づいて適切なSQL方言とドライバーを使用する
