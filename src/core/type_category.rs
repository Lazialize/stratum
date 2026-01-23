// 型カテゴリ分類
//
// カラム型を型カテゴリに分類し、型変更の互換性を検証するための機能を提供します。

use super::schema::ColumnType;

/// 型カテゴリ
///
/// カラム型を大まかなカテゴリに分類します。
/// 型変更の互換性検証に使用されます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCategory {
    /// 数値型 (INTEGER, DECIMAL, FLOAT, DOUBLE)
    Numeric,
    /// 文字列型 (VARCHAR, TEXT, CHAR)
    String,
    /// 日時型 (DATE, TIME, TIMESTAMP)
    DateTime,
    /// バイナリ型 (BLOB)
    Binary,
    /// JSON型 (JSON, JSONB)
    Json,
    /// 真偽値型 (BOOLEAN)
    Boolean,
    /// UUID型 (UUID)
    Uuid,
    /// その他 (ENUM, DialectSpecific)
    Other,
}

/// 型変換の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeConversionResult {
    /// 安全な変換（警告なし）
    Safe,
    /// サイズ縮小など、精度損失の可能性がある変換
    SafeWithPrecisionCheck,
    /// データ損失の可能性がある変換（警告）
    Warning,
    /// 互換性がない変換（エラー）
    Error,
}

impl TypeCategory {
    /// ColumnTypeから型カテゴリを判定
    pub fn from_column_type(column_type: &ColumnType) -> Self {
        match column_type {
            // 数値型
            ColumnType::INTEGER { .. }
            | ColumnType::DECIMAL { .. }
            | ColumnType::FLOAT
            | ColumnType::DOUBLE => TypeCategory::Numeric,

            // 文字列型
            ColumnType::VARCHAR { .. } | ColumnType::TEXT | ColumnType::CHAR { .. } => {
                TypeCategory::String
            }

            // 日時型
            ColumnType::DATE | ColumnType::TIME { .. } | ColumnType::TIMESTAMP { .. } => {
                TypeCategory::DateTime
            }

            // バイナリ型
            ColumnType::BLOB => TypeCategory::Binary,

            // JSON型
            ColumnType::JSON | ColumnType::JSONB => TypeCategory::Json,

            // 真偽値型
            ColumnType::BOOLEAN => TypeCategory::Boolean,

            // UUID型
            ColumnType::UUID => TypeCategory::Uuid,

            // その他（ENUM、方言固有型）
            ColumnType::Enum { .. } | ColumnType::DialectSpecific { .. } => TypeCategory::Other,
        }
    }

    /// 他のカテゴリへの変換結果を判定
    ///
    /// 型変更の互換性マトリクスに基づいて、変換が安全か、警告が必要か、エラーかを判定します。
    pub fn conversion_result(&self, to: &Self) -> TypeConversionResult {
        use TypeCategory::*;
        use TypeConversionResult::*;

        match (self, to) {
            // 同一カテゴリ内の変換（サイズ縮小チェックが必要）
            (Numeric, Numeric)
            | (String, String)
            | (DateTime, DateTime)
            | (Binary, Binary)
            | (Json, Json)
            | (Boolean, Boolean)
            | (Uuid, Uuid) => SafeWithPrecisionCheck,

            // Numeric → 他
            (Numeric, String) => Safe,
            (Numeric, Boolean) => Warning,
            (Numeric, DateTime) | (Numeric, Binary) | (Numeric, Json) | (Numeric, Uuid) => Error,

            // String → 他
            (String, Numeric) | (String, DateTime) | (String, Boolean) => Warning,
            (String, Binary) | (String, Json) | (String, Uuid) => Safe,

            // DateTime → 他
            (DateTime, String) => Safe,
            (DateTime, Numeric)
            | (DateTime, Binary)
            | (DateTime, Json)
            | (DateTime, Boolean)
            | (DateTime, Uuid) => Error,

            // Binary → 他
            (Binary, String) => Safe,
            (Binary, Numeric)
            | (Binary, DateTime)
            | (Binary, Json)
            | (Binary, Boolean)
            | (Binary, Uuid) => Error,

            // Json → 他
            (Json, String) => Safe,
            (Json, Numeric)
            | (Json, DateTime)
            | (Json, Binary)
            | (Json, Boolean)
            | (Json, Uuid) => Error,

            // Boolean → 他
            (Boolean, Numeric) | (Boolean, String) => Safe,
            (Boolean, DateTime) | (Boolean, Binary) | (Boolean, Json) | (Boolean, Uuid) => Error,

            // Uuid → 他
            (Uuid, String) => Safe,
            (Uuid, Numeric)
            | (Uuid, DateTime)
            | (Uuid, Binary)
            | (Uuid, Json)
            | (Uuid, Boolean) => Error,

            // Other は常にSafeWithPrecisionCheck（実行時に判断）
            (Other, _) | (_, Other) => SafeWithPrecisionCheck,
        }
    }

    /// 他のカテゴリへの変換が警告対象かどうか
    pub fn is_warning_conversion(&self, other: &Self) -> bool {
        matches!(self.conversion_result(other), TypeConversionResult::Warning)
    }

    /// 他のカテゴリへの変換がエラー対象かどうか
    pub fn is_error_conversion(&self, other: &Self) -> bool {
        matches!(self.conversion_result(other), TypeConversionResult::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // TypeCategory::from_column_type のテスト
    // ==========================================

    #[test]
    fn test_from_column_type_numeric() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::INTEGER { precision: None }),
            TypeCategory::Numeric
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::DECIMAL {
                precision: 10,
                scale: 2
            }),
            TypeCategory::Numeric
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::FLOAT),
            TypeCategory::Numeric
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::DOUBLE),
            TypeCategory::Numeric
        );
    }

    #[test]
    fn test_from_column_type_string() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::VARCHAR { length: 255 }),
            TypeCategory::String
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::TEXT),
            TypeCategory::String
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::CHAR { length: 10 }),
            TypeCategory::String
        );
    }

    #[test]
    fn test_from_column_type_datetime() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::DATE),
            TypeCategory::DateTime
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::TIME {
                with_time_zone: None
            }),
            TypeCategory::DateTime
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::TIMESTAMP {
                with_time_zone: None
            }),
            TypeCategory::DateTime
        );
    }

    #[test]
    fn test_from_column_type_binary() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::BLOB),
            TypeCategory::Binary
        );
    }

    #[test]
    fn test_from_column_type_json() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::JSON),
            TypeCategory::Json
        );
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::JSONB),
            TypeCategory::Json
        );
    }

    #[test]
    fn test_from_column_type_boolean() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::BOOLEAN),
            TypeCategory::Boolean
        );
    }

    #[test]
    fn test_from_column_type_uuid() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::UUID),
            TypeCategory::Uuid
        );
    }

    #[test]
    fn test_from_column_type_other() {
        assert_eq!(
            TypeCategory::from_column_type(&ColumnType::Enum {
                name: "status".to_string()
            }),
            TypeCategory::Other
        );
    }

    // ==========================================
    // conversion_result のテスト（互換性マトリクス）
    // ==========================================

    #[test]
    fn test_same_category_conversion() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // 同一カテゴリ内の変換はサイズチェックが必要
        assert_eq!(Numeric.conversion_result(&Numeric), SafeWithPrecisionCheck);
        assert_eq!(String.conversion_result(&String), SafeWithPrecisionCheck);
        assert_eq!(
            DateTime.conversion_result(&DateTime),
            SafeWithPrecisionCheck
        );
        assert_eq!(Boolean.conversion_result(&Boolean), SafeWithPrecisionCheck);
        assert_eq!(Uuid.conversion_result(&Uuid), SafeWithPrecisionCheck);
    }

    #[test]
    fn test_numeric_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Numeric → String: Safe
        assert_eq!(Numeric.conversion_result(&String), Safe);

        // Numeric → Boolean: Warning
        assert_eq!(Numeric.conversion_result(&Boolean), Warning);

        // Numeric → DateTime, Binary, Json, Uuid: Error
        assert_eq!(Numeric.conversion_result(&DateTime), Error);
        assert_eq!(Numeric.conversion_result(&Binary), Error);
        assert_eq!(Numeric.conversion_result(&Json), Error);
        assert_eq!(Numeric.conversion_result(&Uuid), Error);
    }

    #[test]
    fn test_string_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // String → Numeric, DateTime, Boolean: Warning
        assert_eq!(String.conversion_result(&Numeric), Warning);
        assert_eq!(String.conversion_result(&DateTime), Warning);
        assert_eq!(String.conversion_result(&Boolean), Warning);

        // String → Binary, Json, Uuid: Safe
        assert_eq!(String.conversion_result(&Binary), Safe);
        assert_eq!(String.conversion_result(&Json), Safe);
        assert_eq!(String.conversion_result(&Uuid), Safe);
    }

    #[test]
    fn test_datetime_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // DateTime → String: Safe
        assert_eq!(DateTime.conversion_result(&String), Safe);

        // DateTime → 他: Error
        assert_eq!(DateTime.conversion_result(&Numeric), Error);
        assert_eq!(DateTime.conversion_result(&Binary), Error);
        assert_eq!(DateTime.conversion_result(&Json), Error);
        assert_eq!(DateTime.conversion_result(&Boolean), Error);
        assert_eq!(DateTime.conversion_result(&Uuid), Error);
    }

    #[test]
    fn test_binary_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Binary → String: Safe
        assert_eq!(Binary.conversion_result(&String), Safe);

        // Binary → 他: Error
        assert_eq!(Binary.conversion_result(&Numeric), Error);
        assert_eq!(Binary.conversion_result(&DateTime), Error);
        assert_eq!(Binary.conversion_result(&Json), Error);
        assert_eq!(Binary.conversion_result(&Boolean), Error);
        assert_eq!(Binary.conversion_result(&Uuid), Error);
    }

    #[test]
    fn test_json_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Json → String: Safe
        assert_eq!(Json.conversion_result(&String), Safe);

        // Json → 他: Error
        assert_eq!(Json.conversion_result(&Numeric), Error);
        assert_eq!(Json.conversion_result(&DateTime), Error);
        assert_eq!(Json.conversion_result(&Binary), Error);
        assert_eq!(Json.conversion_result(&Boolean), Error);
        assert_eq!(Json.conversion_result(&Uuid), Error);
    }

    #[test]
    fn test_boolean_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Boolean → Numeric, String: Safe
        assert_eq!(Boolean.conversion_result(&Numeric), Safe);
        assert_eq!(Boolean.conversion_result(&String), Safe);

        // Boolean → 他: Error
        assert_eq!(Boolean.conversion_result(&DateTime), Error);
        assert_eq!(Boolean.conversion_result(&Binary), Error);
        assert_eq!(Boolean.conversion_result(&Json), Error);
        assert_eq!(Boolean.conversion_result(&Uuid), Error);
    }

    #[test]
    fn test_uuid_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Uuid → String: Safe
        assert_eq!(Uuid.conversion_result(&String), Safe);

        // Uuid → 他: Error
        assert_eq!(Uuid.conversion_result(&Numeric), Error);
        assert_eq!(Uuid.conversion_result(&DateTime), Error);
        assert_eq!(Uuid.conversion_result(&Binary), Error);
        assert_eq!(Uuid.conversion_result(&Json), Error);
        assert_eq!(Uuid.conversion_result(&Boolean), Error);
    }

    #[test]
    fn test_other_category_conversions() {
        use TypeCategory::*;
        use TypeConversionResult::*;

        // Other は常にSafeWithPrecisionCheck
        assert_eq!(Other.conversion_result(&Numeric), SafeWithPrecisionCheck);
        assert_eq!(Other.conversion_result(&String), SafeWithPrecisionCheck);
        assert_eq!(Numeric.conversion_result(&Other), SafeWithPrecisionCheck);
    }

    // ==========================================
    // is_warning_conversion / is_error_conversion のテスト
    // ==========================================

    #[test]
    fn test_is_warning_conversion() {
        use TypeCategory::*;

        assert!(String.is_warning_conversion(&Numeric));
        assert!(String.is_warning_conversion(&Boolean));
        assert!(Numeric.is_warning_conversion(&Boolean));

        assert!(!Numeric.is_warning_conversion(&String));
        assert!(!Boolean.is_warning_conversion(&String));
    }

    #[test]
    fn test_is_error_conversion() {
        use TypeCategory::*;

        assert!(Numeric.is_error_conversion(&DateTime));
        assert!(Numeric.is_error_conversion(&Json));
        assert!(DateTime.is_error_conversion(&Numeric));
        assert!(Json.is_error_conversion(&Numeric));

        assert!(!Numeric.is_error_conversion(&String));
        assert!(!String.is_error_conversion(&Json));
    }
}
