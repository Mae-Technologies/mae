// TODO: delete prelude
pub mod prelude {
    // pub use super::select_update_builder::*;
    pub use super::builder::{BindArgs, Builder, SqlCmd, ToSql, WhereCondition};
    pub use crate::request_context::ContextAccessor;
    pub use anyhow::{Context, anyhow};
    pub use chrono::{DateTime, Utc};
    pub use mae_repo_macro::*;
    pub use num::ToPrimitive;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{Map, Value};
    use sqlx;
    pub use sqlx::Arguments;
    pub use sqlx::postgres::PgArguments;
    pub use sqlx::types::JsonValue as SqlxJson;
    pub use std::fmt;
    pub use std::fmt::Display;

    // TODO:
    // WARN: This nees to be refactored to lib mae/repo/fields
    #[derive(sqlx::Type, Clone, Deserialize, Serialize, Debug)]
    #[sqlx(type_name = "status", rename_all = "lowercase")]
    pub enum DomainStatus {
        Incomplete,
        Active,
        Deleted,
        Archived,
    }
}
pub use builder::KeyAuths;
pub use mae_repo_macro::*;
pub mod builder;
pub mod fields;
pub mod select_update_builder;
