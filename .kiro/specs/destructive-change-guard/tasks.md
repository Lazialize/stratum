# Implementation Plan

## 概要

破壊的変更の安全ガード機能を実装するためのタスクリストです。設計書のMigration Strategyに沿って、4つのフェーズで段階的に実装を進めます。

---

## Phase 1: コアドメインとサービス

- [ ] 1. DestructiveChangeReportモデルの実装
- [ ] 1.1 (P) 破壊的変更レポートの構造体定義
  - 5種類の破壊的変更を保持するフィールドを定義（テーブル削除、カラム削除、カラムリネーム、ENUM削除、ENUM再作成）
  - 削除カラム情報（テーブル名と対象カラムリスト）を表現する補助型を定義
  - リネームカラム情報（テーブル名、旧名、新名）を表現する補助型を定義
  - serde対応のシリアライズ/デシリアライズ属性を追加
  - 空の配列は省略するskip_serializing_if属性を設定
  - _Requirements: 1.2, 1.5_

- [ ] 1.2 (P) レポートのユーティリティメソッド実装
  - 空のレポートを生成するコンストラクタを実装
  - 破壊的変更が含まれているかを判定するメソッドを実装
  - 破壊的変更の総数をカウントするメソッドを実装
  - _Requirements: 1.2, 4.3_

- [ ] 1.3 シリアライゼーションの検証
  - レポートをYAML形式でシリアライズしデシリアライズで復元できることを検証
  - 空のフィールドが省略されることを検証
  - 古いメタデータ形式（フィールドなし）からの読み込みでNoneとなることを検証
  - _Requirements: 1.5_

- [ ] 2. DestructiveChangeDetectorサービスの実装
- [ ] 2.1 破壊的変更検出ロジックの実装
  - スキーマ差分から削除テーブルを抽出しレポートに追加
  - 各テーブル差分から削除カラムを抽出しレポートに追加
  - 各テーブル差分からリネームカラムを抽出しレポートに追加
  - 削除ENUMをレポートに追加
  - ENUM再作成（Recreate種別）をレポートに追加
  - 複数の変更が存在する場合に全て漏れなく検出することを保証
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 2.2 検出サービスのユニットテスト
  - 空のスキーマ差分で空のレポートが返ることを検証
  - 5種類すべての破壊的変更が正しく分類されることを検証
  - 複数テーブル・複数カラムの削除がリストに正しく追加されることを検証
  - 同一スキーマ差分に対して常に同じ結果が返る冪等性を検証
  - _Requirements: 1.1, 1.3_

---

## Phase 2: CLI統合と許可フラグ

- [ ] 3. CLI引数への--allow-destructiveフラグ追加
- [ ] 3.1 (P) generateコマンドへのフラグ追加
  - clap derive macroで--allow-destructiveフラグを定義
  - デフォルト値をfalseに設定（デフォルト拒否の原則）
  - フラグの説明文を追加（破壊的変更の許可）
  - _Requirements: 3.1, 3.4_

- [ ] 3.2 (P) applyコマンドへのフラグ追加
  - clap derive macroで--allow-destructiveフラグを定義
  - デフォルト値をfalseに設定
  - generateと同じ説明文を使用し一貫性を保つ
  - _Requirements: 3.2, 3.4_

- [ ] 4. エラーメッセージフォーマッターの実装
- [ ] 4.1 破壊的変更エラーの整形ロジック
  - エラータイトル「Destructive changes detected」を表示
  - 変更種別ごとにグルーピング表示（Tables to be dropped: ...等）
  - 影響を受けるテーブル・カラム・ENUMの一覧を表示
  - 3つの選択肢（dry-run確認、許可フラグ使用、スキーマ見直し）を提示
  - coloredライブラリで赤色強調表示を適用
  - _Requirements: 2.3, 2.4, 5.1, 5.3, 5.4, 5.5_

- [ ] 4.2 旧メタデータ時の最小限エラー表示
  - マイグレーションバージョンの表示
  - 「Legacy migration format detected」警告バナーを表示
  - --allow-destructive使用の案内を表示
  - 変更種別の詳細は省略（メタデータがないため）
  - _Requirements: 5.2_

- [ ] 4.3 警告メッセージの整形ロジック
  - 許可フラグ指定時の警告表示（エラーではなく警告）
  - 破壊的変更の概要を表示
  - 黄色での警告色表示を適用
  - _Requirements: 3.5_

---

## Phase 3: メタデータ拡張とコマンド統合

- [ ] 5. MigrationMetadataの拡張
- [ ] 5.1 destructive_changesフィールドの追加
  - MigrationMetadataにDestructiveChangeReportをOption型で追加
  - serde(default)属性で古いメタデータとの後方互換性を確保
  - 空オブジェクトも保存する設定（旧メタデータとの判別のため）
  - _Requirements: 1.5, 2.2_

- [ ] 5.2 メタデータ読み込み時の破壊的変更判定
  - destructive_changesフィールドありの場合はその内容で判定
  - destructive_changesフィールドなしの場合は破壊的変更ありとみなす
  - フィールドが空オブジェクトの場合は破壊的変更なしと判定
  - _Requirements: 2.2_

- [ ] 6. generateコマンドへの破壊的変更ガード統合
- [ ] 6.1 破壊的変更検出と拒否メカニズム
  - スキーマ差分検出後にDestructiveChangeDetectorを呼び出し
  - 破壊的変更あり かつ --allow-destructiveなしの場合はエラーで拒否
  - エラーメッセージに影響範囲と次のアクションを含める
  - 破壊的変更なしの場合は通常通りマイグレーション生成を続行
  - _Requirements: 2.1, 2.5_

- [ ] 6.2 --allow-destructive指定時の動作
  - 破壊的変更を含むマイグレーションファイルを生成
  - 警告メッセージを表示（エラーではなく警告）
  - .meta.yamlにdestructive_changesフィールドを保存
  - _Requirements: 3.1, 3.3, 3.5_

- [ ] 6.3 dry-runモードでの破壊的変更プレビュー
  - 「Destructive Changes Detected」セクションをプレビューに追加
  - DROP/RENAMEなどの変更を赤色で強調表示
  - 影響範囲の集計（削除テーブル数、カラム数）を表示
  - 「--allow-destructiveで続行」の指示を表示
  - 既存のリネームプレビュー機能と統合し一貫した表示形式を維持
  - _Requirements: 4.1, 4.2, 4.3, 4.5, 4.6_

- [ ] 7. applyコマンドへの破壊的変更ガード統合
- [ ] 7.1 メタデータからの破壊的変更判定と拒否
  - .meta.yamlのdestructive_changesフィールドを読み取り
  - 破壊的変更あり かつ --allow-destructiveなしの場合はエラーで拒否
  - エラーメッセージにマイグレーションバージョンと変更詳細を含める
  - 旧メタデータの場合は最小限情報のみ表示
  - _Requirements: 2.2, 5.2_

- [ ] 7.2 --allow-destructive指定時の動作
  - 破壊的変更を含むマイグレーションを適用
  - 警告メッセージを表示
  - 古いマイグレーション（メタデータなし）の適用も許可
  - _Requirements: 3.2, 3.3, 3.5_

- [ ] 7.3 dry-runモードでの破壊的変更表示
  - 新メタデータの場合：up.sql内の破壊的SQLを軽量キーワードハイライトで表示
  - DROP/RENAME/ALTER等のキーワードを正規表現で検出し赤色表示
  - 旧メタデータの場合：警告バナーのみ表示（キーワードハイライトなし）
  - 「--allow-destructiveで続行」の指示を表示
  - _Requirements: 4.4, 4.5_

---

## Phase 4: enum_recreate_allowed廃止

- [ ] 8. MigrationPipelineのallow_destructive統合
- [ ] 8.1 enum_recreate_allowedフラグの廃止
  - enum_recreate_allowedの参照箇所をallow_destructiveに置換
  - ENUM再作成・削除の許可をallow_destructiveフラグで制御
  - 既存のENUM再作成ガードロジックを新フラグに移行
  - _Requirements: 3.3_

- [ ] 8.2 互換性警告の実装
  - 設定やスキーマにenum_recreate_allowedフィールドが存在する場合に警告表示
  - 「Use '--allow-destructive' instead」のメッセージを出力
  - フィールドの値は無視し、--allow-destructiveフラグのみで動作を制御
  - _Requirements: 3.3_

---

## Phase 5: 統合テストとE2Eテスト

- [ ] 9. 統合テストの実装
- [ ] 9.1 (P) generateコマンドの統合テスト
  - 破壊的変更を含むスキーマ差分でgenerateがエラーで拒否されることを検証
  - --allow-destructive付きでgenerateが.meta.yamlにdestructive_changesを保存することを検証
  - --dry-runで破壊的変更のプレビューが赤色で表示されることを検証
  - 破壊的変更なしの場合は空のdestructive_changesオブジェクトが保存されることを検証
  - _Requirements: 2.1, 3.1, 4.1, 4.2_

- [ ] 9.2 (P) applyコマンドの統合テスト
  - destructive_changesを含むマイグレーションがエラーで拒否されることを検証
  - --allow-destructive付きでapplyがマイグレーションを適用することを検証
  - 古いマイグレーション（フィールドなし）が破壊的変更ありとして拒否されることを検証
  - 古いマイグレーション + --allow-destructiveで適用成功することを検証
  - _Requirements: 2.2, 3.2_

- [ ] 9.3 (P) ENUM再作成の統合テスト
  - --allow-destructive付きでENUM再作成を含むgenerateが成功することを検証
  - --allow-destructiveなしでENUM再作成がエラーで拒否されることを検証
  - MigrationPipelineがallow_destructiveフラグを参照してENUM再作成を制御することを検証
  - _Requirements: 3.3_

- [ ] 9.4 E2Eワークフローテスト
  - テーブル削除→generateエラー→dry-run確認→--allow-destructive生成→apply --allow-destructive適用のフローを検証
  - カラムリネーム→generate --allow-destructive→.meta.yamlにリネーム情報保存→applyで検出のフローを検証
  - 非破壊的変更のみ→generate→空のdestructive_changes→applyで即適用のフローを検証
  - _Requirements: 2.1, 2.2, 2.5, 3.1, 3.2_

---

## 要件カバレッジ

| 要件ID | タスク | 説明 |
|--------|--------|------|
| 1.1 | 2.1, 2.2 | 5種類の破壊的変更検出 |
| 1.2 | 1.1, 1.2, 2.1 | 影響範囲のリスト化 |
| 1.3 | 2.1, 2.2 | 複数変更の漏れなき検出 |
| 1.4 | 2.1 | 検出サービス化 |
| 1.5 | 1.1, 1.3, 2.1, 5.1 | レポート返却 |
| 2.1 | 6.1, 9.1, 9.4 | generate拒否メカニズム |
| 2.2 | 5.1, 5.2, 7.1, 9.2, 9.4 | apply拒否メカニズム |
| 2.3 | 4.1 | 破壊的変更の一覧表示 |
| 2.4 | 4.1 | 許可フラグの案内 |
| 2.5 | 6.1, 9.4 | 非破壊的変更の通常実行 |
| 3.1 | 3.1, 6.2, 9.1, 9.4 | generate --allow-destructive |
| 3.2 | 3.2, 7.2, 9.2, 9.4 | apply --allow-destructive |
| 3.3 | 6.2, 7.2, 8.1, 8.2, 9.3 | デフォルト拒否のバイパス |
| 3.4 | 3.1, 3.2 | CLI引数の受付 |
| 3.5 | 4.3, 6.2, 7.2 | 警告メッセージ表示 |
| 3.6 | 6.3, 7.3 | dry-runでのプレビュー |
| 4.1 | 6.3, 9.1 | dry-run専用セクション追加 |
| 4.2 | 6.3, 9.1 | 色付き強調表示 |
| 4.3 | 1.2, 6.3 | 影響範囲の集計表示 |
| 4.4 | 7.3 | apply dry-runでの警告表示 |
| 4.5 | 6.3, 7.3 | 続行方法の指示表示 |
| 4.6 | 6.3 | リネームプレビュー統合 |
| 5.1 | 4.1, 6.1 | generate拒否時のエラー詳細 |
| 5.2 | 4.2, 7.1 | apply拒否時のエラー詳細 |
| 5.3 | 4.1 | 色付きエラー表示 |
| 5.4 | 4.1 | 変更種別のグルーピング |
| 5.5 | 4.1 | 選択肢の提示 |

---

generated_at: 2026-01-27T09:15:00Z
