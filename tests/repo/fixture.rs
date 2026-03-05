//! Test fixture for the `RepoExample` domain struct.
//!
//! Provides a minimal `#[schema]`-annotated struct and helper functions that produce
//! canonical insert/update/patch/filter payloads. Tests in `crud.rs` rely on these
//! helpers rather than inlining data, so changes to the schema surface area only need
//! to be updated here.
//!
//! The `#[schema(Ctx, "repoexample")]` macro generates:
//! - `InsertRow` — used with `insert_one` / `insert_many`
//! - `UpdateRow` — used with `update_many` (all fields wrapped in `Option`)
//! - `PatchField` — used with `patch` (enum; each variant holds one field value)
//! - `Field` — used for SELECT projections and WHERE filters
//!
//! `KeyAuths` is implemented manually here with an empty key set because `repoexample`
//! is not tenant-scoped in tests. Production structs override this to enforce
//! `sys_client` filtering.

use crate::common::context::Ctx;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{KeyAuths, ToField};
use mae::repo::macros::schema;
pub use serde_json::Map;
pub use sqlx::types::JsonValue as SqlxJson;

/// Minimal domain struct used as the test subject for the repo CRUD layer.
///
/// Only `value` and `string_value` are declared as user-defined fields; all other
/// columns (`id`, `sys_client`, `status`, `tags`, `sys_detail`, audit fields) are
/// injected by the `#[schema]` macro as standard platform columns.
#[schema(Ctx, "repoexample")]
#[allow(non_snake_case, non_camel_case_types, nonstandard_style)]
pub struct RepoExample {
    pub value: i32,
    pub string_value: String,
}

impl<F: ToField,> KeyAuths<F,> for RepoExample {
    fn keys() -> Vec<FilterOp<F,>,> {
        // No tenant-scoping in tests — all rows are visible regardless of sys_client.
        // Production implementations should return a sys_client filter here.
        // TODO: This needs to actually add the rows.
        Vec::<FilterOp<F,>,>::new()
    }
}

// TODO: fixture methods should be dynamically generated randomly

/// Returns a canonical `InsertRow` with deterministic values.
///
/// `string_value` is set to `"hello_world"` — several tests assert on this exact value,
/// so changing it here will cause those assertions to fail.
pub fn gen_insert_row() -> InsertRow {
    InsertRow {
        sys_client: 1,
        status: DomainStatus::Active,
        value: 1,
        string_value: String::from("hello_world",),
        comment: None,
        tags: SqlxJson::Array(vec![],),
        sys_detail: SqlxJson::Object(Map::new(),),
    }
}

/// Returns a canonical `UpdateRow` with all fields set (non-None).
///
/// Used by `should_update` and the error-path tests that need a valid update payload.
/// All `Option` wrappers are `Some(…)` so the full-row update path is exercised.
pub fn gen_update_row() -> UpdateRow {
    UpdateRow {
        status: Some(DomainStatus::Active,),
        value: Some(1,),
        string_value: Some(String::from("hello_world",),),
        comment: Some(None,),
        tags: Some(SqlxJson::Array(vec![],),),
        sys_detail: Some(SqlxJson::Object(Map::new(),),),
    }
}

/// Returns a set of `PatchField` variants covering a partial field selection:
/// `value`, `comment`, and `status`. Intentionally omits `string_value`, `tags`, and
/// `sys_detail` to exercise the partial-update behaviour of `SqlStatement::Patch` —
/// only the three specified columns should appear in the generated UPDATE SET clause.
pub fn gen_patches() -> Vec<PatchField,> {
    vec![
        PatchField::value(100,),
        PatchField::comment(Some("patching!".into(),),),
        PatchField::status(DomainStatus::Archived,),
    ]
}

/// Returns a filter that targets a non-existent `string_value` pattern.
///
/// Used by patch/update tests that need valid WHERE clause bindings but expect zero
/// matched rows (e.g. `patch_should_return_empty`).
pub fn gen_filters() -> Vec<FilterOp<Field,>,> {
    vec![FilterOp::Begin(Field::string_value, Filter::Like("sdfsdfsdfsd".into(),),)]
}
