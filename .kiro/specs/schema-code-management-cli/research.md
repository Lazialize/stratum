# Research & Design Decisions

## Summary
- **Feature**: `schema-code-management-cli`
- **Discovery Scope**: New Feature (Greenfield)
- **Key Findings**:
  - 既存のスキーマ管理ツールは宣言的アプローチ（Atlas）と増分アプローチ（Flyway/Liquibase）の2つの主流がある
  - Rust実装により単一バイナリ配布、ランタイム依存なし、高パフォーマンスを実現
  - Clap v4（derive機能）がRust CLIのデファクトスタンダード、structoptは統合済み
  - SQLx が PostgreSQL/MySQL/SQLite を統一的に扱える最適な選択肢（コンパイル時SQL検証、pure Rust実装）
  - serde_yaml は非推奨、serde-saphyr が最新の後継（中間構文木なしで直接パース、パフォーマンス最適化）
  - Tokio がエコシステム互換性の観点から非同期ランタイムの最適解

## Research Log

### CLI フレームワーク選定（Rust）
- **Context**: Rustで型安全、高パフォーマンス、単一バイナリ配布可能なCLIを構築
- **Sources Consulted**:
  - [Clap and Structopt Crafting Intuitive Rust CLIs](https://leapcell.io/blog/clap-and-structopt-crafting-intuitive-rust-clis)
  - [Picking an argument parser - Rain's Rust CLI recommendations](https://rust-cli-recommendations.sunshowers.io/cli-parser.html)
  - [clap FAQ](https://docs.rs/clap/latest/clap/_faq/index.html)
- **Findings**:
  - Clap v4: structopt を統合、derive マクロで宣言的にCLI定義可能、Rustエコシステムのデファクトスタンダード
  - structopt: Clap v3以降に統合され、現在はメンテナンスモードのみ
  - Clap の derive 機能により `#[derive(Parser)]` で型安全なCLI引数パースが可能
  - サブコマンド、オプション、引数の検証が compile-time に実施される
- **Implications**:
  - Clap v4 の derive 機能を採用し、宣言的かつ型安全なCLI定義を実現
  - コンパイル時の型チェックにより実行時エラーを大幅に削減
  - 単一バイナリとして配布可能、Node.js等のランタイム依存が不要
  - クロスコンパイルによりLinux/macOS/Windows向けバイナリを容易に生成

### データベースドライバー調査（Rust）
- **Context**: PostgreSQL、MySQL、SQLite の Rust ドライバー選定
- **Sources Consulted**:
  - [SQLx GitHub](https://github.com/launchbadge/sqlx)
  - [Compare Diesel](https://diesel.rs/compare_diesel.html)
  - [Choosing a Rust Database Crate in 2023](https://rust-trends.com/posts/database-crates-diesel-sqlx-tokio-postgress/)
- **Findings**:
  - **SQLx**: async、pure Rust、コンパイル時SQL検証、PostgreSQL/MySQL/SQLite統一サポート、接続プーリング内蔵
  - **Diesel**: ORM、DSL、型安全なクエリビルダー、async版は diesel-async（別crate）
  - **tokio-postgres**: PostgreSQL専用、低レベルAPI、Tokio統合
  - SQLx の PostgreSQL/MySQL ドライバーは pure Rust、unsafe コード不使用
  - SQLx はランタイム選択可能（tokio / async-std / actix）、TLSバックエンド選択可能（native-tls / rustls）
- **Implications**:
  - **SQLx を採用**: 3つのデータベースを統一的に扱え、コンパイル時SQL検証により実行時エラーを削減
  - pure Rust 実装のため、外部ライブラリ依存なしでクロスコンパイル可能
  - 接続プーリング（PgPool/MySqlPool/SqlitePool）が標準搭載
  - マクロ `sqlx::query!` でコンパイル時にデータベーススキーマを検証（オプション、開発時のみ）

### スキーマ差分検出アルゴリズム
- **Context**: YAML スキーマ定義からマイグレーションファイルを自動生成するための差分検出方法
- **Sources Consulted**:
  - [Atlas Database Schema Diff](https://atlasgo.io/declarative/diff)
  - [DBDiff GitHub](https://github.com/DBDiff/DBDiff)
  - [Atlas GitHub](https://github.com/ariga/atlas)
- **Findings**:
  - Atlas は宣言的アプローチを採用し、現在の状態と望ましい状態を比較して差分を生成
  - 差分検出はテーブル、カラム、インデックス、制約の構造的比較を実施
  - タイムスタンプ付きマイグレーションファイルを生成し、up/down スクリプトを含む
- **Implications**:
  - 本ツールでは増分的な差分検出を実装し、前回のスナップショットと現在のYAML定義を比較
  - テーブル追加/削除、カラム追加/削除/変更、インデックス追加/削除、制約追加/削除を検出
  - 差分検出結果から安全なマイグレーション順序を決定（依存関係グラフの構築）

### YAML パース＆バリデーション（Rust）
- **Context**: YAML形式のスキーマ定義ファイルの安全なパースと構造検証
- **Sources Consulted**:
  - [Serde-yaml deprecation alternatives - Rust Forum](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868)
  - [serde-saphyr announcement](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306)
  - [serde_yaml lib.rs](https://lib.rs/crates/serde_yaml)
- **Findings**:
  - **serde_yaml**: 2024-03-25に非推奨化、メンテナンスモードのみ
  - **serde-saphyr**: 2025年9月リリース、中間構文木を構築せずに直接Rust構造体にパース、パフォーマンス最適化
  - **serde_yml**: serde-yamlのフォーク、互換性重視
  - Rust の Serde エコシステムにより、YAML → 強い型付き構造体への自動デシリアライズ
- **Implications**:
  - **serde-saphyr を採用**: 中間構文木なしで直接デシリアライズし、メモリ効率とパフォーマンス向上
  - `#[derive(Deserialize)]` により YAML スキーマ定義を Schema 構造体に自動変換
  - カスタムバリデーター実装により外部キー参照整合性、命名規則をチェック
  - コンパイル時の型安全性により実行時エラーを大幅削減

### トランザクション管理とロールバック
- **Context**: マイグレーション実行時の安全性確保とエラー時のロールバック戦略
- **Sources Consulted**:
  - [PostgreSQL Transactions Documentation](https://www.postgresql.org/docs/current/tutorial-transactions.html)
  - [The Complete Guide to Database Transactions](https://medium.com/@alxkm/the-complete-guide-to-database-transactions-how-commit-and-rollback-really-work-in-mysql-and-36d1ce81b9eb)
  - [Can You Roll Back CREATE TABLE & ALTER TABLE?](https://www.codestudy.net/blog/is-it-possible-to-roll-back-create-table-and-alter-table-statements-in-major-sql-databases/)
- **Findings**:
  - PostgreSQL、MySQL、SQLite は全て DDL 操作をトランザクション内で実行可能
  - Savepoint を使用した部分的なロールバックが可能
  - トランザクションは短く保ち、必要な操作のみを含めることがベストプラクティス
  - エラーハンドリングは常に ROLLBACK を実装すべき
- **Implications**:
  - 各マイグレーションファイルの実行は個別のトランザクション内で実行
  - エラー発生時は自動的に ROLLBACK し、マイグレーション履歴テーブルを更新しない
  - dry-run モードではトランザクションを開始し、最後に必ず ROLLBACK

### マイグレーション管理のベストプラクティス
- **Context**: 業界標準のマイグレーション管理手法の調査
- **Sources Consulted**:
  - [Top Database Schema Migration Tools 2025](https://www.bytebase.com/blog/top-database-schema-change-tool-evolution/)
  - [dbmate GitHub](https://github.com/amacneil/dbmate)
  - [How to Build CI/CD Pipeline for Database Schema Migration](https://www.bytebase.com/blog/how-to-build-cicd-pipeline-for-database-schema-migration/)
- **Findings**:
  - タイムスタンプベースのファイル命名規則が標準（例: 20260121120000_create_users.sql）
  - マイグレーション履歴を専用テーブル（schema_migrations）に記録
  - up/down の両方のスクリプトを含めることで双方向の変更が可能
  - CI/CD パイプラインへの統合が重要
- **Implications**:
  - マイグレーションファイル命名規則: `{timestamp}_{description}.{dialect}.sql`
  - 履歴管理テーブル（schema_migrations）には: version, description, applied_at, checksum を記録
  - マイグレーションの整合性を checksum で検証し、改ざん検出を実施

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Hexagonal (Ports & Adapters) | コアロジックを外部依存から分離し、ポート（インターフェース）とアダプター（実装）で構成 | テスト容易性、データベース間の切り替えが容易、ビジネスロジックの独立性 | 初期実装の複雑さ、小規模プロジェクトではオーバーエンジニアリングの可能性 | 複数のデータベース方言をサポートするため、アダプターパターンが有効 |
| Layered Architecture | CLI → Service → Repository → Database の階層構造 | シンプルで理解しやすい、責任の分離が明確 | 層間の依存が固定的、モック化が困難になる可能性 | 中規模CLIツールには適切なバランス |
| Plugin Architecture | コアシステムにプラグインを追加してデータベースサポートを拡張 | 拡張性が高い、新しいデータベースの追加が容易 | 初期設計の複雑さ、プラグイン管理のオーバーヘッド | 将来的な拡張を考慮すると魅力的だが、v1.0には過剰 |

**選定**: Hexagonal Architecture（ポート＆アダプター）を簡略化した形で採用
- コアドメインロジック（スキーマ解析、差分検出、検証）を純粋な TypeScript で実装
- データベースアクセスは DatabaseAdapter インターフェースを定義し、各 dialect 用の具体実装を提供
- ファイルシステムアクセスも FileSystemPort で抽象化し、テスト容易性を確保

## Design Decisions

### Decision: Rust言語とClap v4を採用

- **Context**: ランタイム依存なし、単一バイナリ配布、高パフォーマンス、型安全性を実現する必要がある
- **Alternatives Considered**:
  1. Node.js + TypeScript + Commander.js - 広く採用、開発速度高、ランタイム依存あり
  2. Go + cobra - 単一バイナリ、クロスコンパイル容易、エコシステムやや弱い
  3. Rust + Clap - 型安全性最強、パフォーマンス最高、学習曲線やや急
- **Selected Approach**: Rust + Clap v4（derive機能）を採用
- **Rationale**:
  - 単一バイナリ配布によりユーザーはRustランタイムのインストール不要
  - コンパイル時の型チェックと所有権システムにより実行時エラーを最小化
  - パフォーマンス要件（1000テーブル/10秒、マイグレーション生成/5秒）を容易に達成
  - Clap の derive マクロによりCLI定義が宣言的かつ型安全
  - クロスコンパイルによりLinux/macOS/Windows向けバイナリを1つのコードベースから生成
- **Trade-offs**:
  - Node.jsエコシステムより学習曲線がやや急だが、型安全性とパフォーマンスで相殺
  - 開発速度は初期段階でやや遅いが、リファクタリングや保守性は高い
  - 非同期処理のランタイム（Tokio）が必要だが、エコシステム成熟済み
- **Follow-up**: Clap の Parser derive と Subcommand derive を活用し、型安全なCLI構造を構築

### Decision: YAML をスキーマ定義フォーマットとして採用

- **Context**: スキーマ定義をコードとして管理するためのフォーマット選定
- **Alternatives Considered**:
  1. JSON - 厳密だがコメント不可、可読性が低い
  2. YAML - 可読性が高い、コメント可能、階層構造が直感的
  3. TypeScript - 型安全だが実行時のパースが必要
- **Selected Approach**: YAML を採用し、JSON Schema でバリデーション
- **Rationale**:
  - 開発者にとって読みやすく、Git での差分確認が容易
  - コメントを含めることができ、スキーマ定義に説明を追加可能
  - JSON Schema エコシステムを活用してバリデーションが可能
- **Trade-offs**:
  - TypeScript のような型チェックはエディタレベルでは提供されない
  - YAML パーサーの脆弱性リスクがあるため、信頼できるライブラリ（js-yaml）を使用
- **Follow-up**: YAML スキーマ定義のサンプルとベストプラクティスをドキュメント化

### Decision: 差分検出アルゴリズムを増分スナップショット比較方式で実装

- **Context**: スキーマ定義の変更を検出してマイグレーションファイルを生成する必要がある
- **Alternatives Considered**:
  1. 宣言的アプローチ（Atlas型） - 常にデータベースの現在状態と比較
  2. 増分スナップショット方式 - 前回のYAML定義を保存して比較
  3. Git履歴ベース - Gitコミット間の差分を検出
- **Selected Approach**: 増分スナップショット方式を採用
- **Rationale**:
  - データベース接続なしでも差分検出が可能（オフライン開発可能）
  - 前回のスナップショットをマイグレーション履歴として保存することで監査証跡を確保
  - Gitに依存せず、任意のバージョン管理システムで動作
- **Trade-offs**:
  - スナップショットファイルの管理が必要（.stratum/snapshots/ ディレクトリ）
  - 手動でスキーマを変更した場合、ツールが検出できない（ドキュメントで警告）
- **Follow-up**: スナップショットの整合性検証とクリーンアップ機能を実装

### Decision: マイグレーション履歴テーブルとして schema_migrations を使用

- **Context**: 適用済みマイグレーションを追跡し、未適用のマイグレーションを検出する必要がある
- **Alternatives Considered**:
  1. schema_migrations テーブル（業界標準）
  2. 専用の管理データベース
  3. ファイルシステムベースの履歴（.stratum/history.json）
- **Selected Approach**: schema_migrations テーブルを各データベースに作成
- **Rationale**:
  - Flyway、Dbmate などの業界標準と一貫性がある
  - データベース自体に履歴を保存することで、環境間の同期が容易
  - トランザクション内で履歴更新とスキーマ変更を原子的に実行可能
- **Trade-offs**:
  - 初回実行時に schema_migrations テーブルを作成する必要がある
  - テーブルが存在しない場合のエラーハンドリングが必要
- **Follow-up**: schema_migrations テーブルのスキーマを各 dialect に合わせて定義

### Decision: SQLx による統一的なデータベースアクセス

- **Context**: PostgreSQL、MySQL、SQLite の SQL 方言の違いを管理し、型安全なクエリを実現する必要がある
- **Alternatives Considered**:
  1. SQLx - コンパイル時SQL検証、3DB統一サポート、pure Rust
  2. Diesel - ORM、DSL、型安全、async版は別crate
  3. 個別ドライバー（tokio-postgres/mysql_async/rusqlite）- 各DBに最適化、統一性なし
- **Selected Approach**: SQLx を採用し、trait ベースのアダプターパターンで dialect 差異を吸収
- **Rationale**:
  - SQLx により PostgreSQL/MySQL/SQLite を統一的な API で扱える
  - `Database` trait により接続プールやトランザクション管理を抽象化
  - コンパイル時SQL検証（オプション）により開発時の品質向上
  - pure Rust 実装のためクロスコンパイルが容易
  - async/await ネイティブサポート、Tokio統合
- **Trade-offs**:
  - Diesel のような高レベル DSL は使用しないが、SQL生成の柔軟性を確保
  - コンパイル時SQL検証にはデータベース接続が必要（CI環境で対応）
- **Follow-up**: `DatabasePort` trait を定義し、各 dialect 用の実装を提供（PostgreSqlAdapter/MySqlAdapter/SqliteAdapter）

## Risks & Mitigations

- **Risk 1: スキーマ差分検出の複雑性** - 外部キー制約の依存関係を正しく解決できない可能性
  - **Mitigation**: トポロジカルソートアルゴリズムを使用して依存関係グラフを構築し、安全な順序でマイグレーションを生成

- **Risk 2: データ損失リスク** - DROP 操作が含まれるマイグレーションを誤って実行する可能性
  - **Mitigation**: 破壊的な操作（DROP TABLE、DROP COLUMN）には確認プロンプトを表示し、--force フラグなしでは実行しない

- **Risk 3: トランザクションサポートの違い** - SQLite は一部の ALTER TABLE 操作をサポートしていない
  - **Mitigation**: SQLite アダプターでは CREATE TABLE + データコピー + DROP TABLE の手法を使用してカラム変更を実現

- **Risk 4: パフォーマンス問題** - 大規模スキーマ（1000テーブル以上）の解析に時間がかかる可能性
  - **Mitigation**: ストリーミングパーサーを使用し、メモリ効率的に YAML を解析。並列処理でテーブル検証を高速化

- **Risk 5: セキュリティ脆弱性** - YAML パーサーの脆弱性によるコード実行リスク
  - **Mitigation**: 信頼できるライブラリ（js-yaml）を使用し、安全なパースオプション（safeLoad）を使用。定期的な依存関係の更新

### Decision: Tokio 非同期ランタイムを採用

- **Context**: データベースI/Oは非同期処理が必須、ランタイム選定が必要
- **Alternatives Considered**:
  1. Tokio - エコシステム最大、パフォーマンス高、多機能
  2. async-std - 標準ライブラリ風API、エコシステム小、メンテナンス縮小
  3. smol - 軽量、シンプル、エコシステム小
- **Selected Approach**: Tokio を採用
- **Rationale**:
  - SQLx が Tokio をネイティブサポート（互換性最高）
  - Rust エコシステムのデファクトスタンダード（Axum/Tonic/Reqwest等が依存）
  - TokioConf 2026開催など、活発なコミュニティとエコシステム
  - `#[tokio::main]` により簡単なセットアップ
- **Trade-offs**:
  - async-std との互換性はないが、本プロジェクトでは影響なし
  - 多機能だが、基本的な使い方はシンプル
- **Follow-up**: `tokio = { version = "1", features = ["full"] }` を Cargo.toml に追加

## References

- [Atlas: Manage your database schema as code](https://atlasgo.io/) - 宣言的スキーマ管理のリファレンス実装
- [dbmate GitHub](https://github.com/amacneil/dbmate) - 軽量なマイグレーションツールのアーキテクチャ参考
- [PostgreSQL Transactions Documentation](https://www.postgresql.org/docs/current/tutorial-transactions.html) - トランザクション管理のベストプラクティス
- [Clap Documentation](https://docs.rs/clap/latest/clap/) - Rust CLI フレームワーク公式ドキュメント
- [SQLx GitHub](https://github.com/launchbadge/sqlx) - Rust SQL Toolkit、コンパイル時SQL検証
- [serde-saphyr Rust Forum](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306) - YAML パーサー（serde-yaml後継、中間構文木なし）
- [Tokio Documentation](https://tokio.rs/) - 非同期ランタイム公式ドキュメント
- [Top Database Schema Migration Tools 2025](https://www.bytebase.com/blog/top-database-schema-change-tool-evolution/) - 業界動向とベストプラクティス
