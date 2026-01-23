# Gap Analysis: column-type-migration

**作成日**: 2026-01-23T00:00:00Z
**仕様ID**: column-type-migration
**分析対象**: 既存コードベースと要件の実装ギャップ

---

## 1. 現状調査 (Current State Investigation)

### 1.1 関連資産

#### 主要コンポーネント

**スキーマ差分モデル** (`src/core/schema_diff.rs`)
- `ColumnDiff` が `old_column` / `new_column` を保持し、`ColumnChange::TypeChanged` を生成
- 型変更の検出結果は `TableDiff.modified_columns` に集約
- `ColumnChange` は `old_type` / `new_type` を `Debug` 文字列で保持

**スキーマ差分検出** (`src/services/schema_diff_detector.rs`)
- `detect_column_diff()` でカラムの差分を比較し `ColumnDiff` を生成
- 型変更・NULL変更・デフォルト変更・AUTO_INCREMENT変更を区別
- 複数カラムの変更を同時に検出可能

**マイグレーション生成** (`src/services/migration_generator.rs`)
- 追加カラム・追加インデックス・テーブル追加/削除に対応
- `modified_columns` / `removed_columns` / `constraints` 変更は未処理
- `SqlGenerator` を利用するが、`ALTER COLUMN TYPE` 生成は未実装

**SQLジェネレーター** (`src/adapters/sql_generator/*`)
- `SqlGenerator` は `CREATE TABLE` / `CREATE INDEX` / `ALTER TABLE ADD CONSTRAINT` のみ
- 方言別の `map_column_type()` で型マッピングは実装済み
- SQLite は ALTER TABLE の制約により `ADD CONSTRAINT` を無視

**CLI (generate/apply/validate)**
- `generate` はマイグレーションファイル生成のみ（dry-run なし）
- `apply --dry-run` は既存の `up.sql` を表示
- `validate` はスキーマ検証だが警告の表示は未実装

### 1.2 既存の慣習とパターン

- 差分検出は `SchemaDiffDetector` → `SchemaDiff` のパイプラインで統一
- SQL生成は `MigrationGenerator` がまとめ、方言差分は `SqlGenerator` 実装で吸収
- 検証は `SchemaValidatorService` と `ValidationWarning` に集約
- CLI は各コマンドで出力整形を内包（共通出力ユーティリティは未整備）

### 1.3 統合ポイント

- YAML → `SchemaParserService` → `Schema` → `SchemaDiffDetector` → `MigrationGenerator`
- 方言判定は `Config.dialect` を各サービスに注入
- エラー/警告モデルは `core/error.rs` に定義済み

---

## 2. 要件実現性分析 (Requirements Feasibility)

### 2.1 技術的要求事項

#### Requirement 1: カラム型変更の検出
**必要な機能**:
- 型変更を差分として検出し、前後の型情報を保持

**現状のギャップ**:
- ✅ `SchemaDiffDetector` と `ColumnDiff` が型変更を検出
- ✅ `ColumnDiff` に `old_column` / `new_column` が保持される
- ⚠️ `ColumnChange::TypeChanged` の `old_type/new_type` が `Debug` 文字列で、表示用途に弱い

#### Requirement 2: 型変更マイグレーションSQLの生成
**必要な機能**:
- PostgreSQL: `ALTER TABLE ... ALTER COLUMN ... TYPE ...`
- MySQL: `ALTER TABLE ... MODIFY COLUMN ...`
- SQLite: テーブル再作成による型変更

**現状のギャップ**:
- ❌ `MigrationGenerator` が `modified_columns` を処理していない
- ❌ `SqlGenerator` に ALTER COLUMN 系のAPIがない
- ❌ SQLiteの再作成ロジックが存在しない
- ⚠️ `get_column_type_string()` は `ColumnType::to_sql_type()` に依存し、方言差分（例: auto_increment）を完全には表現できない

#### Requirement 3: 方言間の型マッピング
**必要な機能**:
- 共通型の方言マッピングと、方言固有制約へのエラーハンドリング

**現状のギャップ**:
- ✅ `ColumnType::to_sql_type()` と各 `SqlGenerator::map_column_type()` にマッピング実装あり
- ❌ 型変更時の方言制約チェックは未実装
- ⚠️ 方言別の ALTER COLUMN 構文組み立てが存在しない

#### Requirement 4: 型変更の検証
**必要な機能**:
- 危険な型変更の警告・不正変換のエラー

**現状のギャップ**:
- ❌ 変更差分に対する検証サービスが存在しない
- ❌ `ValidationWarning` の出力が CLI で表示されない
- ⚠️ 互換性ルール（VARCHAR→INTEGER等）の定義が未作成

#### Requirement 5: dry-runモードでの型変更プレビュー
**必要な機能**:
- マイグレーション生成時のSQLプレビュー
- 型変更の前後型情報の表示

**現状のギャップ**:
- ⚠️ `apply --dry-run` は既存SQLを表示できるが「生成時のプレビュー」ではない
- ❌ `generate` コマンドに dry-run モードがない
- ❌ 型変更の before/after を人間向けに表示する機能がない

### 2.2 制約と未知事項

**アーキテクチャ上の制約**:
- `SqlGenerator` トレイトは CREATE 系のみで ALTER 系 API が不足
- SQLite は ALTER COLUMN TYPE 非対応のため、テーブル再作成が必要

**未知事項（Research Needed）**:
1. SQLiteの安全なテーブル再作成手順（制約/インデックス/トリガーを含む再構築手順）
2. 各方言での ALTER COLUMN TYPE の互換性・制約（例: MySQLの MODIFY COLUMNでの NULL/DEFAULT扱い）
3. 型変更の互換性ルールの定義（危険/許可/禁止の分類基準）

---

## 3. 実装アプローチオプション

### Option A: 既存コンポーネント拡張（Extend Existing Components）

**概要**: `MigrationGenerator` と `SqlGenerator` に ALTER COLUMN 系のAPIを追加して既存パイプラインに統合

**変更対象**:
- `src/adapters/sql_generator/mod.rs`: ALTER COLUMN API追加
- `src/adapters/sql_generator/{postgres,mysql,sqlite}.rs`: 方言別ALTER実装
- `src/services/migration_generator.rs`: `modified_columns` のSQL生成
- `src/services/schema_diff_detector.rs`: 既存利用（変更不要）

**Trade-offs**:
- ✅ 既存パイプラインに沿うため影響範囲が限定的
- ✅ 差分検出モデルの再利用が可能
- ❌ SQLite再作成ロジックが複雑化し、Generatorが肥大化しやすい
- ❌ CLIの警告/エラー表示は別途実装が必要

### Option B: 新規差分マイグレーション層の導入（Create New Components）

**概要**: 型変更専用のサービス (`ColumnTypeMigrationService`) を新設し、SQL生成と検証を集中管理

**新規コンポーネント**:
- `src/services/column_type_migration.rs`: 型変更SQL/検証ロジック
- `src/adapters/sql_generator/*`: ALTER生成は委譲

**Trade-offs**:
- ✅ 型変更の検証・警告・SQL生成を一箇所に集約できる
- ✅ SQLite再作成や安全性チェックを分離可能
- ❌ 新規ファイル/インターフェースが増える
- ❌ 既存の `MigrationGenerator` との境界設計が必要

### Option C: ハイブリッド（Hybrid Approach）

**概要**: `MigrationGenerator` を拡張しつつ、型変更の検証とSQLite再作成だけを専用サービスに切り出す

**組み合わせ戦略**:
- `MigrationGenerator` は `ColumnChange::TypeChanged` を検出して委譲
- 検証ロジックとSQLiteの再作成は新規サービスに分離

**Trade-offs**:
- ✅ 既存構造を活かしつつ複雑部を分離できる
- ✅ SQLiteや互換性検証の拡張が容易
- ❌ 境界設計の初期コストが増える

---

## 4. 実装複雑性とリスク評価

### 4.1 工数見積もり

- **Option A**: **M (3-7 days)**
  - 理由: ALTER API追加と方言別実装で完結するがSQLiteの再作成がボトルネック
- **Option B**: **L (1-2 weeks)**
  - 理由: 新規サービス設計と既存ジェネレーターとの統合が必要
- **Option C**: **M〜L (3-10 days)**
  - 理由: 既存拡張 + 複雑部の分離で段階実装が可能

### 4.2 リスク評価

- **Option A**: Medium
  - 理由: Generator肥大化とSQLite再作成の失敗リスク
- **Option B**: Medium
  - 理由: 新規インターフェース設計と統合手順の不確実性
- **Option C**: Medium-Low
  - 理由: 既存構造を維持しつつリスク部位を切り出せる

---

## 5. 設計フェーズへの推奨事項 (Recommendations)

### 5.1 推奨アプローチ

**Option C (ハイブリッド)** を推奨
- 既存 `MigrationGenerator` の流れを維持しつつ、SQLite再作成と互換性検証を専用化できるため

### 5.2 設計フェーズで決定すべき事項

1. **ALTER COLUMN APIの形**
   - `SqlGenerator` に `generate_alter_column_type(...)` を追加するか
2. **SQLite再作成の責務分離**
   - 生成ロジックをサービス化するか、SqlGenerator内で吸収するか
3. **互換性ルールの定義**
   - 危険/許可/禁止の分類（例: VARCHAR→INTEGER は警告、JSONB→TEXTは許可など）
4. **CLI出力の統合**
   - `generate` か `validate` に警告表示を組み込むか

### 5.3 設計フェーズで実施すべきリサーチ

1. SQLiteでの型変更の公式なベストプラクティス
2. MySQL/PostgreSQLでの ALTER COLUMN TYPE の制約（デフォルト/NOT NULL/インデックス/外部キーの影響）
3. 既存の `ValidationWarning::Compatibility` の利用方針

---

## 6. 要件とコンポーネントのマッピング

| 要件 | 既存コンポーネント | ギャップ | 備考 |
|------|-------------------|---------|------|
| Req 1.1 型変更検出 | `SchemaDiffDetector` | ✅ Reusable | `ColumnDiff` で検出済み |
| Req 1.2 旧/新型の保持 | `ColumnDiff` | ✅ Reusable | `old_column/new_column` が保持 |
| Req 1.3 複数変更検出 | `SchemaDiffDetector` | ✅ Reusable | 複数カラム対応 |
| Req 1.4 他変更との区別 | `ColumnChange` | ✅ Reusable | 変更種別を区別 |
| Req 2.1 ALTER生成 (PG) | `MigrationGenerator` | ❌ Missing | ALTER COLUMN TYPE未実装 |
| Req 2.2 ALTER生成 (MySQL) | `MigrationGenerator` | ❌ Missing | MODIFY COLUMN未実装 |
| Req 2.3 ALTER生成 (SQLite) | `SqliteSqlGenerator` | ❌ Missing | テーブル再作成未実装 |
| Req 3.1 標準型マッピング | `ColumnType::to_sql_type()` | ✅ Reusable | 変更時の再利用が必要 |
| Req 3.2 方言制約エラー | - | ❌ Missing | 互換性検証未実装 |
| Req 3.3 マッピング適用 | `SqlGenerator` | ⚠️ Constraint | ALTER系API拡張が必要 |
| Req 4.1 危険変更警告 | `ValidationWarning` | ❌ Missing | Diffベース検証が未実装 |
| Req 4.2 精度低下警告 | `ValidationWarning` | ❌ Missing | ルール定義が必要 |
| Req 4.3 不正変換エラー | `ValidationError` | ❌ Missing | 新規バリデータが必要 |
| Req 5.1 dry-run表示 | `apply --dry-run` | ⚠️ Constraint | 生成時プレビューは未対応 |
| Req 5.2 ファイル作成抑止 | `generate` | ❌ Missing | dry-runフラグが必要 |
| Req 5.3 型差分表示 | CLI | ❌ Missing | before/after表示機能が必要 |

**凡例**:
- ✅ **Reusable**: 既存コンポーネントを流用可能
- ⚠️ **Constraint**: 既存コンポーネントの拡張が必要
- ❌ **Missing**: 新規作成が必要

---

## 7. まとめ

- 型変更の検出は既存実装でほぼ満たされているが、SQL生成と検証は未実装
- 方言別のALTER文生成とSQLite再作成が最大のギャップ
- CLIのdry-run/警告表示は現状不足しており、設計段階で出力方針を固める必要がある

**技術的リスク**: Medium
**実装工数**: M〜L
