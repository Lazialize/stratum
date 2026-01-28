// マイグレーションディレクトリ読み込みの共通ユーティリティ
//
// apply, rollback, status コマンドで共通して使用する
// マイグレーションディレクトリのスキャン・パースロジックを提供します。

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// マイグレーションディレクトリをスキャンし、(version, description, path) のタプルを返す
///
/// ディレクトリ名の形式: `{timestamp}_{description}`
/// - `.` で始まるディレクトリはスキップ
/// - `_` で分割できないディレクトリはスキップ
/// - バージョン順（昇順）にソートして返す
pub fn load_available_migrations(migrations_dir: &Path) -> Result<Vec<(String, String, PathBuf)>> {
    let mut migrations = Vec::new();

    let entries = fs::read_dir(migrations_dir)
        .with_context(|| format!("Failed to read migrations directory: {:?}", migrations_dir))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("Invalid directory name"))?;

            // .で始まるディレクトリはスキップ
            if dir_name.starts_with('.') {
                continue;
            }

            // ディレクトリ名から version と description を抽出
            // 形式: {timestamp}_{description}
            let parts: Vec<&str> = dir_name.splitn(2, '_').collect();
            if parts.len() == 2 {
                let version = parts[0].to_string();
                let description = parts[1].to_string();
                migrations.push((version, description, path));
            }
        }
    }

    // バージョン順にソート
    migrations.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(migrations)
}
