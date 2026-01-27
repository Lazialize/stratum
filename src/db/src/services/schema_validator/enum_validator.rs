// ENUM定義の検証

use crate::core::config::Dialect;
use crate::core::error::{ValidationError, ValidationResult};
use crate::core::schema::Schema;

/// ENUM定義の検証
///
/// - PostgreSQL以外の方言でENUMが定義されていないか確認
/// - ENUM値が空でないか確認
/// - ENUM値に重複がないか確認
pub fn validate_enums(schema: &Schema, dialect: Option<Dialect>) -> ValidationResult {
    let mut result = ValidationResult::new();

    // ENUMはPostgreSQL専用
    if let Some(dialect) = dialect {
        if !matches!(dialect, Dialect::PostgreSQL) && !schema.enums.is_empty() {
            result.add_error(ValidationError::Constraint {
                message: format!(
                    "ENUM definitions are only supported in PostgreSQL (current: {})",
                    dialect
                ),
                location: None,
                suggestion: Some("Remove ENUM definitions or switch to PostgreSQL".to_string()),
            });
        }
    }

    // ENUM定義の検証
    for enum_def in schema.enums.values() {
        if enum_def.values.is_empty() {
            result.add_error(ValidationError::Constraint {
                message: format!("ENUM '{}' has no values defined", enum_def.name),
                location: None,
                suggestion: Some("Define at least one ENUM value".to_string()),
            });
            continue;
        }

        let mut seen = std::collections::HashSet::new();
        for value in &enum_def.values {
            if !seen.insert(value) {
                result.add_error(ValidationError::Constraint {
                    message: format!("ENUM '{}' has duplicate value '{}'", enum_def.name, value),
                    location: None,
                    suggestion: Some("Remove duplicate values".to_string()),
                });
                break;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::EnumDefinition;

    #[test]
    fn test_validate_enums_empty_values() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec![],
        });

        let result = validate_enums(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("no values")));
    }

    #[test]
    fn test_validate_enums_duplicate_values() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "active".to_string()],
        });

        let result = validate_enums(&schema, None);

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("duplicate")));
    }

    #[test]
    fn test_validate_enums_non_postgres_dialect() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string()],
        });

        let result = validate_enums(&schema, Some(Dialect::MySQL));

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_string().contains("PostgreSQL")));
    }

    #[test]
    fn test_validate_enums_valid() {
        let mut schema = Schema::new("1.0".to_string());
        schema.add_enum(EnumDefinition {
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
        });

        let result = validate_enums(&schema, Some(Dialect::PostgreSQL));

        assert!(result.is_valid());
    }
}
