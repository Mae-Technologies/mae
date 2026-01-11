use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Clone, Deserialize, Serialize, Debug)]
#[sqlx(type_name = "status", rename_all = "lowercase")]
pub enum DomainStatus {
    Incomplete,
    Active,
    Deleted,
    Archived,
}
