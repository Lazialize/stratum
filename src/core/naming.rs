// 命名ポリシー
//
// アプリケーション名と関連パスの単一ソースを提供します。

/// 現行アプリケーション名
pub const APP_NAME: &str = "strata";

/// 既定の設定ファイル名
pub const CONFIG_FILE: &str = ".strata.yaml";

/// 既定の状態ディレクトリ
pub const STATE_DIR: &str = ".strata";

/// バイナリ名
pub const BINARY_NAME: &str = "strata";

/// 命名プロファイル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingProfile {
    pub app_name: String,
    pub config_path: String,
    pub state_dir: String,
    pub binary_name: String,
}

/// 命名ポリシー
pub trait NamingPolicy {
    fn current() -> NamingProfile;
}

/// 既定の命名ポリシー
pub struct DefaultNamingPolicy;

impl NamingPolicy for DefaultNamingPolicy {
    fn current() -> NamingProfile {
        NamingProfile {
            app_name: APP_NAME.to_string(),
            config_path: CONFIG_FILE.to_string(),
            state_dir: STATE_DIR.to_string(),
            binary_name: BINARY_NAME.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_naming_profile() {
        let profile = DefaultNamingPolicy::current();

        assert_eq!(profile.app_name, "strata");
        assert_eq!(profile.config_path, ".strata.yaml");
        assert_eq!(profile.state_dir, ".strata");
        assert_eq!(profile.binary_name, "strata");
    }
}
