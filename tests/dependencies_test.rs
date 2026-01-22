/// 依存関係が正しく設定されているかをテスト
///
/// このテストは、プロジェクトに必要なクレートが正しくインポートできることを確認します。
/// TDDサイクルのREDフェーズとして、まずこのテストが失敗することを確認します。

#[cfg(test)]
mod dependencies_tests {
    /// clapクレートがインポートできることを確認
    #[test]
    fn test_clap_dependency() {
        // clapのderive機能が使えることを確認
        use clap::Parser;

        #[derive(Parser)]
        struct TestArgs {
            #[arg(short, long)]
            test: Option<String>,
        }

        // 構造体が正しく定義できることを確認（型名の存在確認）
        let type_name = std::any::type_name::<TestArgs>();
        assert!(type_name.contains("TestArgs"));
    }

    /// tokioクレートがインポートできることを確認
    #[test]
    fn test_tokio_dependency() {
        // tokio::main属性が使えることを確認
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert!(true);
        });
    }

    /// serdeクレートがインポートできることを確認
    #[test]
    fn test_serde_dependency() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestStruct {
            name: String,
        }

        let test = TestStruct {
            name: "test".to_string(),
        };

        assert_eq!(test.name, "test");
    }

    /// anyhowクレートがインポートできることを確認
    #[test]
    fn test_anyhow_dependency() {
        use anyhow::{anyhow, Result};

        fn test_func() -> Result<()> {
            Err(anyhow!("test error"))
        }

        assert!(test_func().is_err());
    }

    /// thiserrorクレートがインポートできることを確認
    #[test]
    fn test_thiserror_dependency() {
        use thiserror::Error;

        #[derive(Debug, Error)]
        enum TestError {
            #[error("test error: {0}")]
            Test(String),
        }

        let err = TestError::Test("hello".to_string());
        assert_eq!(err.to_string(), "test error: hello");
    }

    /// sha2クレートがインポートできることを確認
    #[test]
    fn test_sha2_dependency() {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(b"test");
        let result = hasher.finalize();

        assert!(!result.is_empty());
    }
}
