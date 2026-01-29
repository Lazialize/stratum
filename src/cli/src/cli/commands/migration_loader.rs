// マイグレーションディレクトリ読み込みの共通ユーティリティ
//
// apply, rollback, status コマンドで共通して使用する
// マイグレーションディレクトリのスキャン・パースロジックを提供します。

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// タイムスタンプ形式が有効かどうかを検証する
///
/// 有効な形式: 数字のみで構成された14桁の文字列 (YYYYMMDDHHmmss)
fn is_valid_timestamp(s: &str) -> bool {
    s.len() == 14 && s.chars().all(|c| c.is_ascii_digit())
}

/// マイグレーションディレクトリをスキャンし、(version, description, path) のタプルを返す
///
/// ディレクトリ名の形式: `{timestamp}_{description}`
/// - `.` で始まるディレクトリはスキップ
/// - `_` で分割できないディレクトリは警告を出力してスキップ
/// - タイムスタンプが不正な形式の場合は警告を出力してスキップ
/// - 重複バージョンが検出された場合はエラーを返す
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
            if parts.len() != 2 {
                eprintln!(
                    "Warning: Skipping directory '{}': does not match expected format '{{timestamp}}_{{description}}'",
                    dir_name
                );
                continue;
            }

            let version = parts[0].to_string();
            let description = parts[1].to_string();

            // タイムスタンプ形式の検証
            if !is_valid_timestamp(&version) {
                eprintln!(
                    "Warning: Skipping directory '{}': version '{}' is not a valid 14-digit timestamp (YYYYMMDDHHmmss)",
                    dir_name, version
                );
                continue;
            }

            migrations.push((version, description, path));
        }
    }

    // バージョン順にソート
    migrations.sort_by(|a, b| a.0.cmp(&b.0));

    // 重複バージョンの検出
    for window in migrations.windows(2) {
        if window[0].0 == window[1].0 {
            return Err(anyhow!(
                "Duplicate migration version detected: '{}' (directories: '{}' and '{}')",
                window[0].0,
                window[0].1,
                window[1].1,
            ));
        }
    }

    Ok(migrations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_valid_timestamp() {
        assert!(is_valid_timestamp("20260121120000"));
        assert!(!is_valid_timestamp("not_a_timestamp"));
        assert!(!is_valid_timestamp("2026012112")); // 10桁
        assert!(!is_valid_timestamp("202601211200001")); // 15桁
        assert!(!is_valid_timestamp("2026012112000a")); // 非数字を含む
    }

    #[test]
    fn test_load_valid_migrations() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_create_users")).unwrap();
        fs::create_dir(temp_dir.path().join("20260121120001_create_posts")).unwrap();

        let migrations = load_available_migrations(temp_dir.path()).unwrap();
        assert_eq!(migrations.len(), 2);
        assert_eq!(migrations[0].0, "20260121120000");
        assert_eq!(migrations[1].0, "20260121120001");
    }

    #[test]
    fn test_skip_invalid_directory_name() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_create_users")).unwrap();
        fs::create_dir(temp_dir.path().join("nodescription")).unwrap();

        let migrations = load_available_migrations(temp_dir.path()).unwrap();
        assert_eq!(migrations.len(), 1);
    }

    #[test]
    fn test_skip_invalid_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_valid")).unwrap();
        fs::create_dir(temp_dir.path().join("not_a_migration")).unwrap();

        let migrations = load_available_migrations(temp_dir.path()).unwrap();
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].0, "20260121120000");
    }

    #[test]
    fn test_duplicate_version_error() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_create_users")).unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_create_posts")).unwrap();

        let result = load_available_migrations(temp_dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate migration version"));
    }

    #[test]
    fn test_skip_hidden_directories() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join(".hidden")).unwrap();
        fs::create_dir(temp_dir.path().join("20260121120000_valid")).unwrap();

        let migrations = load_available_migrations(temp_dir.path()).unwrap();
        assert_eq!(migrations.len(), 1);
    }
}
