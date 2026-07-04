use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Debug, Clone, Deserialize, Serialize)]
#[sqlx(type_name = "status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DomainStatus {
    Incomplete,
    Active,
    Deleted,
    Archived
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_lowercase_status_variants() {
        for (raw, expected) in [
            ("active", DomainStatus::Active),
            ("deleted", DomainStatus::Deleted),
            ("archived", DomainStatus::Archived),
            ("incomplete", DomainStatus::Incomplete)
        ] {
            let parsed: DomainStatus = serde_json::from_value(serde_json::json!(raw)).unwrap();
            assert!(
                matches!(parsed, ref e if std::mem::discriminant(e) == std::mem::discriminant(&expected))
            );
        }
    }
}
