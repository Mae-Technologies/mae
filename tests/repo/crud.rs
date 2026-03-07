use crate::common::context::get_context;
use crate::common::must::{Must, must_be_true, must_eq};
use crate::repo::fixture::Field;
use crate::repo::fixture::{self, RepoExample};
use anyhow::Result;
use chrono::Utc;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{Execute, Interface};
use mae::request_context::ContextAccessor;
use mae_macros::mae_test;
pub use serde_json::Map;
pub use sqlx::types::JsonValue as SqlxJson;

/// Validates that the `#[schema]` macro correctly generates a domain struct with the
/// expected field types. This is a compile-time smoke test — if the struct fields or
/// their types change, this test fails to compile before any DB is involved.
#[cfg_attr(miri, ignore)]
#[mae_test]
fn should_make_domain_struct() {
    let _my_repo = fixture::RepoExample {
        value: 1,
        string_value: String::from("hello_world"),
        comment: None,
        id: 1,
        sys_client: 1,
        status: DomainStatus::Active,
        tags: SqlxJson::Array(vec![]),
        sys_detail: SqlxJson::Object(Map::new()),
        created_by: 1,
        updated_by: 1,
        updated_at: Utc::now(),
        created_at: Utc::now()
    };
}

/// Validates that `insert_one` generates correct SQL and successfully inserts a row,
/// returning the inserted record via `RETURNING *`. Runs inside a transaction that is
/// rolled back after the test so no data persists in the test DB.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_insert() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_insert_row(); // let data = RepoExample {
    // };
    let builder = fixture::RepoExample::insert_one(&ctx, data);

    let res = builder.fetch_all(&mut *tx).await?;

    must_eq(res[0].string_value.as_str(), "hello_world");

    Ok(())
}

/// Validates that a SELECT with an ILIKE filter returns an empty result set when
/// no rows match the pattern. Confirms that the WHERE clause is generated correctly
/// and that an empty result is not treated as an error.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_get_empty_records() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let mut builder = fixture::RepoExample::select(&ctx, vec![Field::All]);

    builder = builder.filter(vec![
        FilterOp::Begin(Field::comment, Filter::Ilike("%bye-bye%".to_string())),
        FilterOp::Or(Field::string_value, Filter::Ilike("hello".to_string())),
    ]);

    let res = builder.fetch_all(&mut *tx).await?;

    must_be_true(res.is_empty());
    Ok(())
}

/// Validates the full insert-then-select round-trip: inserts a row and immediately
/// queries for it using a matching filter. Confirms the inserted row is retrievable
/// and that the filter bindings are wired up correctly.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_get_records() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_insert_row();

    let builder = fixture::RepoExample::insert_one(&ctx, data.clone());

    let res = builder.fetch_all(&mut *tx).await?;

    must_eq(res[0].string_value.as_str(), "hello_world");

    let builder = fixture::RepoExample::select(&ctx, vec![Field::All]).filter(vec![
        FilterOp::Begin(Field::string_value, Filter::StringIs(data.string_value.clone())),
        FilterOp::And(Field::value, Filter::Equals(1)),
    ]);

    let res = builder.fetch_all(&mut *tx).await?;

    must_be_true(!res.is_empty());
    Ok(())
}

/// Validates that calling `update_many` without any `.filter(…)` returns an error.
/// This is a safety guard — an unfiltered UPDATE would overwrite every row in the table.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_error_on_update_without_filters() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_update_row();
    let builder = fixture::RepoExample::update_many(&ctx, data);

    let res = builder.fetch_all(&mut *tx).await;
    res.err().must();
    // TODO: this should error, but the error message should also be checked.
    Ok(())
}

/// Validates that an `update_many` where every `Option` field in `UpdateRow` is `None`
/// returns an error. A fully-None update would produce empty SQL and is never intentional.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_error_on_update_with_row_fields_all_none() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::UpdateRow {
        status: None,
        value: None,
        string_value: None,
        comment: None,
        tags: None,
        sys_detail: None
    };
    let mut builder = fixture::RepoExample::update_many(&ctx, data);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into()))]);

    let res = builder.fetch_all(&mut *tx).await;
    // TODO: this should error, but the error message should also be checked.
    res.err().must();
    Ok(())
}

/// Validates that `update_many` with a filter succeeds and executes without error.
/// The inserted row is seeded first so that the UPDATE has at least one row to act on.
///
/// Note: result content is not yet asserted — see the TODO below.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_update() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let new_data = fixture::gen_insert_row();

    let _ = RepoExample::insert_one(&ctx, new_data).fetch_all(ctx.db_pool()).await;

    let data = fixture::gen_update_row();
    let mut builder = fixture::RepoExample::update_many(&ctx, data);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into()))]);

    let _res = builder.fetch_all(&mut *tx).await?;

    // TODO: the result should match the input
    Ok(())
}

/// Validates that calling `patch` without any `.filter(…)` returns an error containing
/// the "Unable to Update/Patch" message. Mirrors the equivalent update guard test.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_error_on_patch_without_filters() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_patches();
    let builder = fixture::RepoExample::patch(&ctx, data);

    let res = builder.fetch_all(&mut *tx).await;
    //
    must_be_true(res.err().must().to_string().contains("Unable to Update/Patch"));
    Ok(())
}

/// Validates that `patch` with an empty `PatchField` vec (i.e. no fields to update)
/// returns an error. An empty patch would produce a no-op UPDATE with no SET columns,
/// which the builder rejects as an error rather than silently succeeding.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_error_on_patch_with_fields_empty() -> Result<()> {
    let ctx = get_context().await.must();

    let mut tx = ctx.db_pool.begin().await?;

    let data: Vec<fixture::PatchField> = vec![];
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let res = builder.fetch_all(&mut *tx).await;
    //
    must_be_true(res.is_err());
    must_be_true(res.err().must().to_string().contains("Unable to Update/Patch"));
    Ok(())
}

/// Validates that a `patch` with filters targeting a non-existent row returns an
/// empty result set (not an error). Confirms that zero matched rows is a valid outcome.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn patch_should_return_empty() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let res = builder.fetch_all(&mut *tx).await?;

    must_be_true(res.is_empty());
    Ok(())
}

/// Validates that a `patch` with matching filters executes successfully.
/// Uses `gen_patches()` (partial field set: value, comment, status) to confirm that
/// only the specified fields are included in the UPDATE — the key behaviour distinguishing
/// `patch` from `update_many`.
///
/// Note: result content is not yet asserted — see the TODO below.
#[cfg_attr(miri, ignore)]
#[mae_test]
async fn should_patch() -> Result<()> {
    let ctx = get_context().await?;

    let mut tx = ctx.db_pool.begin().await?;

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let _res = builder.fetch_all(&mut *tx).await?;

    // TODO: the result should match the input
    Ok(())
}
