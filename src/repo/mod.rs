pub mod prelude {
    pub use super::select_update_builder::*;
    pub use crate::request_context::ContextAccessor;
    pub use anyhow::{Context, anyhow};
    pub use chrono::{DateTime, Utc};
    pub use mae_repo_macro::*;
    pub use num::ToPrimitive;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{Map, Value};
    use sqlx;
    pub use sqlx::Arguments;
    pub use sqlx::types::JsonValue as SqlxJson;
    pub use std::fmt;
    pub use std::fmt::Display;

    #[derive(sqlx::Type, Clone, Deserialize, Serialize, Debug)]
    #[sqlx(type_name = "status", rename_all = "lowercase")]
    pub enum DomainStatus {
        Incomplete,
        Active,
        Deleted,
        Archived,
    }

    #[allow(dead_code)]
    trait ToI32 {
        fn to_i32(&self) -> Result<i32, anyhow::Error>;
    }

    impl ToI32 for u64 {
        fn to_i32(&self) -> Result<i32, anyhow::Error> {
            ToPrimitive::to_i32(self).ok_or_else(|| anyhow!("unable to convert i64 to u32."))
        }
    }
}
pub mod select_update_builder;
pub mod builder;
