# Requirements Document

## Introduction
本ドキュメントは、Stratumコードベースのアーキテクチャ統合リファクタリングに関する要件を定義します。主な目的は、方言依存ロジックの集約、重複排除、責務分離の明確化、およびコード品質の向上です。このリファクタリングにより、保守性、拡張性、テスト容易性が向上し、将来的な機能追加やバグ修正のコストが低減されます。

## Project Description (Input)
リファクタリング
Refactoring 方針

方言依存ロジックを集約して二重実装を減らす（型マッピングの重複があるので共通ヘルパに集約）sqlite.rs, sqlite_table_recreator.rs, export.rs
マイグレーション生成の重複を排除し、共通パイプライン＋フック方式へ寄せる（with/without schemas の分岐を統合）migration_generator.rs
export コマンドの責務分離（DB introspection / 変換 / 出力の分割）で境界を明確化 export.rs, database.rs, schema_serializer.rs
バリデーションを粒度分割し、単一関数の巨大化を避ける（enum/型/制約/参照/警告を分離）schema_validator.rs
SQL文字列組み立ての直接埋め込みを最小化し、bind 可能な経路へ整理（移行履歴の書き込み/削除が対象）database_migrator.rs
DTO変換の往復ロジックを共通化し、片側変更のズレを防ぐ schema_parser.rs, schema_serializer.rs, dto.rs

## Requirements

### Requirement 1: 方言依存ロジックの集約
**Objective:** As a 開発者, I want 型マッピングや方言固有のロジックを共通ヘルパに集約したい, so that 二重実装を排除し、新しい方言の追加やバグ修正を一箇所で行えるようにする

#### Acceptance Criteria
1. The Stratum shall 各方言（PostgreSQL, MySQL, SQLite）の型マッピングロジックを単一の共通モジュールに集約する
2. When 新しいデータ型を追加する場合, the Stratum shall 共通型マッピングモジュールへの変更のみで全方言に対応できる
3. The Stratum shall sqlite.rs, sqlite_table_recreator.rs, export.rs に分散している型変換ロジックの重複を排除する
4. While 方言固有の動作が必要な場合, the Stratum shall 共通インターフェースに対するカスタム実装としてフック可能にする
5. The Stratum shall 既存の全てのユニットテストおよび統合テストが引き続きパスする

### Requirement 2: マイグレーション生成パイプラインの統合
**Objective:** As a 開発者, I want マイグレーション生成の分岐ロジックを統合したい, so that コードの複雑性を減らし、一貫した生成プロセスを実現する

#### Acceptance Criteria
1. The Stratum shall with/without schemas の分岐を統一したパイプライン方式で処理する
2. When マイグレーションを生成する場合, the Stratum shall 共通パイプラインを経由して方言固有のフックを適用する
3. The Stratum shall migration_generator.rs の条件分岐を削減し、単一責任原則に準拠させる
4. If パイプラインの特定ステージでエラーが発生した場合, the Stratum shall 発生箇所を明確にしたエラーメッセージを返す
5. The Stratum shall 既存のマイグレーション生成機能の出力結果を変更しない（後方互換性の維持）

### Requirement 3: export コマンドの責務分離
**Objective:** As a 開発者, I want export コマンドの責務を明確に分離したい, so that 各コンポーネントを独立してテスト・保守できるようにする

#### Acceptance Criteria
1. The Stratum shall export 処理を以下の3つのレイヤーに分離する: DB introspection（データベース情報取得）, 変換（内部モデルへのマッピング）, 出力（YAMLシリアライズ）
2. The Stratum shall export.rs, database.rs, schema_serializer.rs 間の依存関係を明確化し、循環依存を排除する
3. When DB introspection を実行する場合, the Stratum shall データベース固有のロジックを adapters 層に閉じ込める
4. The Stratum shall 変換ロジックを services 層に配置し、出力フォーマットから独立させる
5. The Stratum shall 各レイヤーを個別にユニットテスト可能な設計とする

### Requirement 4: バリデーションロジックの粒度分割
**Objective:** As a 開発者, I want バリデーションを目的別に分割したい, so that 巨大な関数を避け、テストとデバッグを容易にする

#### Acceptance Criteria
1. The Stratum shall schema_validator.rs の検証ロジックを以下のカテゴリに分割する: enum検証, 型検証, 制約検証, 参照整合性検証, 警告生成
2. The Stratum shall 各検証カテゴリを独立した関数またはモジュールとして実装する
3. When 複数のバリデーションエラーが発生した場合, the Stratum shall 全てのエラーを収集して一括で報告する
4. The Stratum shall 単一のバリデーション関数が50行を超えないように構成する
5. While バリデーションを実行する場合, the Stratum shall エラーと警告を明確に区別して返す

### Requirement 5: SQL文字列組み立ての安全化
**Objective:** As a 開発者, I want SQL文字列の直接埋め込みを最小化したい, so that SQLインジェクションのリスクを低減し、コードの安全性を向上させる

#### Acceptance Criteria
1. The Stratum shall database_migrator.rs における移行履歴の書き込み/削除処理でパラメータバインディングを使用する
2. The Stratum shall 文字列フォーマットによるSQL組み立てをbind可能なクエリビルダー方式に置き換える
3. If 動的なテーブル名やカラム名が必要な場合, the Stratum shall 許可リストによるバリデーションを実施する
4. The Stratum shall sqlx のコンパイル時クエリ検証機能を最大限活用する
5. The Stratum shall 文字列補間によるSQL組み立てをコードレビューで検出可能な形式に制限する

### Requirement 6: DTO変換ロジックの統一
**Objective:** As a 開発者, I want DTO変換の往復ロジックを共通化したい, so that パース↔シリアライズ間の不整合を防ぎ、一方の変更が他方に自動反映されるようにする

#### Acceptance Criteria
1. The Stratum shall schema_parser.rs と schema_serializer.rs のDTO変換ロジックを共通モジュールに集約する
2. The Stratum shall dto.rs における型定義と変換ロジックの一貫性を保証する
3. When Schema を YAML にシリアライズし再パースした場合, the Stratum shall 元の Schema と完全に同一のオブジェクトを復元する（ラウンドトリップ整合性）
4. The Stratum shall 変換ロジックのテストでラウンドトリッププロパティを検証する
5. If 新しいフィールドを Schema に追加した場合, the Stratum shall パースとシリアライズの両方が単一箇所の変更で対応できる

## Non-Functional Requirements

### Requirement 7: 後方互換性の維持
**Objective:** As a ユーザー, I want リファクタリング後も既存の動作が維持されることを保証したい, so that アップグレード時に既存のワークフローが破壊されない

#### Acceptance Criteria
1. The Stratum shall 既存の公開APIシグネチャを変更しない
2. The Stratum shall 既存のYAMLスキーマフォーマットとの互換性を維持する
3. The Stratum shall 生成されるマイグレーションSQLの出力形式を変更しない
4. The Stratum shall 既存の152以上のユニットテストおよび27以上のテストスイートが全てパスする

### Requirement 8: コード品質基準の遵守
**Objective:** As a 開発者, I want リファクタリングがRustのベストプラクティスに準拠することを保証したい, so that 技術的負債を増やさず、チームのコーディング規約を維持する

#### Acceptance Criteria
1. The Stratum shall cargo fmt による自動フォーマットに準拠する
2. The Stratum shall cargo clippy で警告が0件であることを維持する
3. The Stratum shall 不要な .clone() の使用を避け、借用を優先する
4. The Stratum shall unwrap()/expect() を本番コードで使用せず、適切なエラーハンドリングを行う
5. The Stratum shall 各モジュールの公開APIにドキュメントコメントを付与する（変更箇所のみ）

## Implementation Approach

### 推奨アプローチ: Option C（ハイブリッド）

ギャップ分析の結果に基づき、以下のハイブリッドアプローチを採用する。

#### 新規モジュール化（責務分離を優先）
| 対象 | 新規モジュール | 理由 |
|------|---------------|------|
| 方言型マッピング | `src/adapters/type_mapping.rs` | 重複が広範囲に分散しており、共通化による効果が大きい |
| DTO変換 | `src/services/dto_converter.rs` | ラウンドトリップ整合性を単一箇所で保証するため |

#### 既存拡張（変更リスクを低減）
| 対象 | 拡張方針 | 理由 |
|------|---------|------|
| export コマンド | `export.rs` 内で責務を関数分離、段階的に adapters 層へ切り出し | API変更を最小化しつつ責務分離を実現 |
| マイグレーション生成 | `migration_generator.rs` の分岐を統合、フック構造を追加 | 既存出力互換性の維持が必須 |
| バリデーション | `schema_validator.rs` 内で関数分割 | 単一ファイル内での整理で十分 |
| SQL組み立て | `database_migrator.rs` の該当箇所をbind方式へ置換 | 影響範囲が限定的 |

#### 設計フェーズでの調査事項
1. **sqlx::query! の利用可否**: AnyPool/複数方言/CI環境での compile-time 検証
2. **型マッピングの配置層**: adapters 層に置くか services 層に置くか（依存方向の検討）
3. **SQLite再作成ロジック**: フォールバック値の共通化範囲

#### トレードオフ
- ✅ 責務分離が明確になりテスト容易性が向上
- ✅ 既存コードへの影響を最小化しリスクを低減
- ❌ 新規モジュールと既存コードの整合性調整が必要
- ❌ 段階的移行による一時的な重複発生の可能性
