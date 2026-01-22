# Requirements Document

## Introduction
StratumにおけるPostgreSQLのENUM型サポートを、スキーマ定義・マイグレーション生成・適用/ロールバック・エクスポートの各ワークフローで一貫して扱えるようにする。

## Requirements

### Requirement 1: ENUM型のスキーマ定義と検証
**Objective:** As a バックエンドエンジニア, I want PostgreSQLのENUM型をYAMLスキーマで定義したい, so that スキーマをコードとして一貫管理できる

#### Acceptance Criteria
1.1 The Stratum CLI shall allow PostgreSQLのENUM型を型名と値の一覧としてスキーマに定義できる
1.2 When ENUM型が定義されている, the Stratum CLI shall 空の値リストを拒否する
1.3 If ENUM型の値に重複が含まれる, the Stratum CLI shall 検証エラーとして報告する
1.4 If カラムが未定義のENUM型を参照している, the Stratum CLI shall 検証エラーとして報告する
1.5 Where PostgreSQL以外の方言が選択されている, the Stratum CLI shall PostgreSQLのENUM型定義を無効として扱う

### Requirement 2: ENUM型に関するマイグレーション生成
**Objective:** As a DevOps/SREチーム, I want ENUM型の変更をマイグレーションとして生成したい, so that 環境間で安全に同期できる

#### Acceptance Criteria
2.1 When PostgreSQL方言でENUM型が新規に定義されている, the Stratum CLI shall その型を作成するDDLをマイグレーションに含める
2.2 When PostgreSQL方言でENUM型の値が変更されている, the Stratum CLI shall 変更後の定義に一致するDDLをマイグレーションに含める
2.3 When PostgreSQL方言でENUM型が削除されている, the Stratum CLI shall 不要となる型を削除するDDLをマイグレーションに含める
2.4 If ENUM型定義に変更がない, the Stratum CLI shall ENUM型に関するDDLを生成しない

### Requirement 3: ENUM型を含む適用・ロールバック
**Objective:** As a バックエンドエンジニア, I want ENUM型を含むマイグレーションを安全に適用・巻き戻ししたい, so that 破壊的な不整合を避けられる

#### Acceptance Criteria
3.1 When ENUM型を含むマイグレーションを適用する, the Stratum CLI shall エラーが発生した場合に適用失敗として報告する
3.2 If ENUM型のDDL適用に失敗した, the Stratum CLI shall 対象マイグレーションを適用済みとして記録しない
3.3 When ENUM型を含むマイグレーションをロールバックする, the Stratum CLI shall 直前のENUM型定義に戻るDDLを実行する

### Requirement 4: ENUM型のエクスポートと再利用
**Objective:** As a バックエンドエンジニア, I want 既存PostgreSQLからENUM型定義を取り込んで再利用したい, so that 既存DBをスキーマコードへ移行できる

#### Acceptance Criteria
4.1 When PostgreSQLからスキーマをエクスポートする, the Stratum CLI shall ENUM型の定義と値をスキーマに含める
4.2 When ENUM型をエクスポートする, the Stratum CLI shall 値の順序を保持する
4.3 When エクスポートされたスキーマを再度読み込む, the Stratum CLI shall 同等のENUM型定義として解釈する
