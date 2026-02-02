# Requirements Document

## Introduction
本仕様は、Strata CLIに「check」コマンドを追加し、既存の「validate」と「generate --dry-run」を連携させた安全な検証フローを提供するための要求事項を定義する。

## Requirements

### Requirement 1: コマンドの目的と振る舞い
**Objective:** As a Strata CLI利用者, I want 既存のvalidateとgenerate --dry-runを一括実行できるcheckコマンド, so that 安全にスキーマ整合性と生成結果を事前確認できる

#### Acceptance Criteria
1. **1.1** When ユーザーがcheckコマンドを実行したとき, the check command shall validateを実行する
2. **1.2** When validateが成功したとき, the check command shall generate --dry-run相当の処理を実行する
3. **1.3** If validateが失敗したとき, the check command shall generate --dry-run相当の処理を実行しない
4. **1.4** The check command shall 既存のvalidateおよびgenerate --dry-runの意味論に一致する結果を提供する

### Requirement 2: 入力と設定の継承
**Objective:** As a Strata CLI利用者, I want checkコマンドが既存コマンドと同等の入力・設定を受け付ける, so that 既存の運用設定を変えずに利用できる

#### Acceptance Criteria
1. **2.1** When ユーザーがcheckコマンドにスキーマ入力や設定を指定したとき, the check command shall validateに同等の入力・設定を適用する
2. **2.2** When ユーザーがcheckコマンドにスキーマ入力や設定を指定したとき, the check command shall generate --dry-run相当の処理に同等の入力・設定を適用する
3. **2.3** The check command shall 既存のvalidateおよびgenerate --dry-runで利用可能な主要な入力経路（設定ファイル・スキーマパス等）を受け付ける

### Requirement 3: 出力と結果表示
**Objective:** As a Strata CLI利用者, I want checkコマンドの結果が分かりやすく表示される, so that 検証結果と生成結果を迅速に判断できる

#### Acceptance Criteria
1. **3.1** When validateが成功したとき, the check command shall 検証成功であることを明示的に表示する
2. **3.2** When validateが成功しgenerate --dry-run相当の処理が成功したとき, the check command shall 生成結果の概要を表示する
3. **3.3** If validateが失敗したとき, the check command shall 失敗理由を利用者が確認できる形で表示する
4. **3.4** If generate --dry-run相当の処理が失敗したとき, the check command shall 失敗理由を利用者が確認できる形で表示する

### Requirement 4: 終了コードと失敗時挙動
**Objective:** As a Strata CLI利用者, I want checkコマンドの終了コードが結果に対応している, so that 自動化パイプラインで判定できる

#### Acceptance Criteria
1. **4.1** When validateとgenerate --dry-run相当の処理が成功したとき, the check command shall 成功を示す終了コードで終了する
2. **4.2** If validateが失敗したとき, the check command shall 失敗を示す終了コードで終了する
3. **4.3** If generate --dry-run相当の処理が失敗したとき, the check command shall 失敗を示す終了コードで終了する

### Requirement 5: 非破壊性
**Objective:** As a Strata CLI利用者, I want checkコマンドが実際のマイグレーション適用を行わない, so that 事前確認を安全に実行できる

#### Acceptance Criteria
1. **5.1** The check command shall 実際のマイグレーション適用を行わない
2. **5.2** While checkコマンドが実行中である間, the check command shall スキーマやデータベースに対する破壊的変更を行わない
