//! Typed async repository layer over SQLx/Postgres.
//!
//! The repository layer gives domain structs a type-safe CRUD API without
//! hand-written SQL.  Annotate a struct with `#[schema]` (from
//! [`macros::schema`]) and it gains:
//!
//! - `InsertRow` / `UpdateRow` / `PatchField` / `Field` generated types
//! - Blanket implementations of [`implement::Interface`] for `select`,
//!   `insert_one`, `update_many`, and `patch`
//! - Row-level auth key filtering via [`implement::KeyAuths`]
//!
//! # Quick start
//!
//! Add `#[schema]` to a domain struct:
//!
//! ```text
//! #[schema]
//! pub struct User {
//!     pub id: i32,
//!     pub name: String,
//! }
//! ```
//!
//! Then call `User::select(&ctx, vec![Field::All])` etc. from your handlers.

pub mod default;

mod build;
mod into_filter;
mod map_util;
// mod sql_parts;
mod type_def;

pub mod filter {
    use super::*;
    pub use map_util::{Filter, FilterOp};
}

pub mod implement {
    use super::*;
    pub use build::{Execute, Interface, KeyAuths};
    pub use type_def::ToField;
}

pub mod macros {
    pub use mae_macros::schema;
    pub use mae_macros::schema_root;
}

pub mod __private__ {
    use super::*;
    pub use build::Build;
    // TODO: AsSqlParts should be a type;
    pub use into_filter::IntoMaeFilter;
    pub use map_util::{AsSqlParts, BindArgs, ToSqlParts};
}
