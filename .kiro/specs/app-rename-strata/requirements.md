# Requirements Document

## Introduction
本機能は、セキュリティソフトによる誤検知を回避するために、アプリケーション名称をstratumからstrataへ変更する。CLIコマンド名、バイナリ名、設定ファイル、状態ディレクトリ、ドキュメント表記を一貫して更新する。

## Requirements

### Requirement 1: CLIコマンド名と実行名の変更
**Objective:** As a CLI利用者, I want strataというコマンド名で実行できる, so that セキュリティソフトにブロックされずに利用できる

#### Acceptance Criteria
1. When ユーザーが`strata`コマンドを実行したとき, the Strata CLI shall 既存の主要機能（init/generate/apply/rollback/status/validate/export）を同等に実行できる
2. The Strata CLI shall ヘルプ表示とバージョン表示で`strata`をアプリ名として表示する

### Requirement 2: 設定ファイルと状態ディレクトリのリネーム
**Objective:** As a プロジェクト利用者, I want 設定ファイルと状態ディレクトリがstrataに統一される, so that 旧名称による誤検知や混在を避けられる

#### Acceptance Criteria
1. When 新規に初期化したとき, the Strata CLI shall `.strata.yaml`を生成する
2. When 初期化または実行時の状態保存が必要なとき, the Strata CLI shall `.strata/`配下に状態やログを保存する
3. The Strata CLI shall 新しい出力や更新を旧名称のパス（`.stratum.yaml`/`.stratum/`）に書き込まない

### Requirement 3: ユーザー向け表示とドキュメントの統一
**Objective:** As a ドキュメント利用者, I want 表記がstrataに統一される, so that 誤解なく参照・共有できる

#### Acceptance Criteria
1. The Strata CLI shall 端末出力・エラーメッセージ・利用案内で`strata`を使用する
2. When サンプルやREADME/BUILDING/CONTRIBUTING等を参照したとき, the Strata CLI shall `strata`表記に統一された内容を提供する

### Requirement 4: パッケージ/バイナリ/配布物の名称変更
**Objective:** As a リリース担当, I want 配布物の名称がstrataになる, so that セキュリティソフトのブロック対象から外せる

#### Acceptance Criteria
1. The Strata CLI shall ビルド成果物の実行ファイル名を`strata`とする
2. The Strata CLI shall パッケージ識別子（クレート名や配布メタデータ）を`strata`として公開する
3. When 配布手順やインストール手順が参照されるとき, the Strata CLI shall `strata`を前提とした記述を提供する
