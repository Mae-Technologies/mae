use crate::common::context::{context_with_companies, keyed_context};
use crate::repo::fixture::Field;
use crate::repo::fixture::{self, RepoExample};
use anyhow::Result;
use chrono::Utc;
use mae::repo::prelude::*;
use mae::testing::must::{Must, must_be_true, must_eq};
use mae_macros::mae_test;
pub use serde_json::Map;
pub use sqlx::types::JsonValue as SqlxJson;

/// Validates that the `#[schema]` macro correctly generates a domain struct with the
/// expected field types. This is a compile-time smoke test — if the struct fields or
/// their types change, this test fails to compile before any DB is involved.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
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
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_insert() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_insert_row(); // let data = RepoExample {
    // };
    let builder = fixture::RepoExample::insert_one(&ctx, data);

    let res = builder.fetch_all(WithExecutor::Transaction).await?;

    must_eq(res[0].string_value.as_str(), "hello_world");

    Ok(())
}

/// Validates that a SELECT with an ILIKE filter returns an empty result set when
/// no rows match the pattern. Confirms that the WHERE clause is generated correctly
/// and that an empty result is not treated as an error.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_get_empty_records() -> Result<()> {
    let ctx = keyed_context().await?;

    let mut builder = fixture::RepoExample::select(&ctx, vec![Field::All]);

    builder = builder.filter(vec![
        FilterOp::Begin(Field::comment, Filter::Ilike("%bye-bye%".to_string())),
        FilterOp::Or(Field::string_value, Filter::Ilike("hello".to_string())),
    ]);

    let res = builder.fetch_all(WithExecutor::Transaction).await?;

    must_be_true(res.is_empty());
    Ok(())
}

/// Validates the full insert-then-select round-trip: inserts a row and immediately
/// queries for it using a matching filter. Confirms the inserted row is retrievable
/// and that the filter bindings are wired up correctly.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_get_records() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_insert_row();

    let builder = fixture::RepoExample::insert_one(&ctx, data.clone());

    let res = builder.fetch_all(WithExecutor::Transaction).await?;

    must_eq(res[0].string_value.as_str(), "hello_world");

    let builder = fixture::RepoExample::select(&ctx, vec![Field::All]).filter(vec![
        FilterOp::Begin(Field::string_value, Filter::StringIs(data.string_value.clone())),
        FilterOp::And(Field::value, Filter::Equals(1)),
    ]);

    let res = builder.fetch_all(WithExecutor::Transaction).await?;

    must_be_true(!res.is_empty());
    Ok(())
}

/// An update with no caller filter is still limited to the required key
/// `sys_client` in 1, 2, 3. The SQL is inspected and not executed.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn update_without_caller_filter_keeps_the_required_key() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_update_row();
    let builder = fixture::RepoExample::update(&ctx, data);
    let rendered = format!("{builder:?}");

    must_be_true(rendered.contains("_x_.sys_client = ANY("));
    must_eq(rendered.matches("OR _x_.sys_client = ").count(), 0);
    Ok(())
}

/// Validates that an `update_many` where every `Option` field in `UpdateRow` is `None`
/// returns an error. A fully-None update would produce empty SQL and is never intentional.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_error_on_update_with_row_fields_all_none() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::UpdateRow {
        status: None,
        value: None,
        string_value: None,
        comment: None,
        tags: None,
        sys_detail: None
    };
    let mut builder = fixture::RepoExample::update(&ctx, data);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into()))]);

    let res = builder.fetch_all(WithExecutor::Transaction).await;
    // TODO: this should error, but the error message should also be checked.
    res.err().must();
    Ok(())
}

/// Validates that `update_many` with a filter succeeds and executes without error.
/// The inserted row is seeded first so that the UPDATE has at least one row to act on.
///
/// Note: result content is not yet asserted — see the TODO below.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_update() -> Result<()> {
    let ctx = keyed_context().await?;

    let new_data = fixture::gen_insert_row();

    let _ = RepoExample::insert_one(&ctx, new_data).fetch_all(WithExecutor::Transaction).await;

    let data = fixture::gen_update_row();
    let mut builder = fixture::RepoExample::update(&ctx, data);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into()))]);

    let _ = builder.fetch_all(WithExecutor::Transaction).await?;

    // TODO: the result should match the input
    Ok(())
}

/// A patch with no caller filter is still limited to the required key
/// `sys_client` in 1, 2, 3. The SQL is inspected and not executed.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn patch_without_caller_filter_keeps_the_required_key() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_patches();
    let builder = fixture::RepoExample::patch(&ctx, data);
    let rendered = format!("{builder:?}");

    must_be_true(rendered.contains("_x_.sys_client = ANY("));
    must_eq(rendered.matches("OR _x_.sys_client = ").count(), 0);
    Ok(())
}

/// Validates that `patch` with an empty `PatchField` vec (i.e. no fields to update)
/// returns an error. An empty patch would produce a no-op UPDATE with no SET columns,
/// which the builder rejects as an error rather than silently succeeding.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_error_on_patch_with_fields_empty() -> Result<()> {
    let ctx = keyed_context().await.must();

    let data: Vec<fixture::PatchField> = vec![];
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let res = builder.fetch_all(WithExecutor::Transaction).await;
    //
    must_be_true(res.is_err());
    must_be_true(res.err().must().to_string().contains("Unable to Update/Patch"));
    Ok(())
}

/// Validates that a `patch` with filters targeting a non-existent row returns an
/// empty result set (not an error). Confirms that zero matched rows is a valid outcome.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn patch_should_return_empty() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let res = builder.fetch_all(WithExecutor::Transaction).await?;

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
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn should_patch() -> Result<()> {
    let ctx = keyed_context().await?;

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let _ = builder.fetch_all(WithExecutor::Transaction).await?;

    // TODO: the result should match the input
    Ok(())
}

/// The required key is `sys_client` in 1, 2, 3. A row for company 9 is stored
/// and then omitted from the select.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn required_key_hides_rows_outside_the_list() -> Result<()> {
    let ctx = keyed_context().await?;
    let marker = format!("key-{}", uuid::Uuid::new_v4().simple());

    for (company, label) in [(1, "one"), (2, "two"), (3, "three"), (9, "nine")] {
        let mut row = fixture::gen_insert_row();
        row.sys_client = company;
        row.string_value = format!("{marker}-{label}");
        RepoExample::insert_one(&ctx, row).fetch_all(WithExecutor::Transaction).await?;
    }

    let rows = RepoExample::select(&ctx, vec![Field::All])
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like(format!("{marker}%")))])
        .fetch_all(WithExecutor::Transaction)
        .await?;

    let mut companies: Vec<i32> = rows
        .iter()
        .filter(|row| row.string_value.starts_with(&marker))
        .map(|row| row.sys_client)
        .collect();
    companies.sort();
    must_eq(companies, vec![1, 2, 3]);
    must_be_true(rows.iter().all(|row| matches!(row.sys_client, 1 | 2 | 3)));

    let hidden: i64 = ctx
        .pg_context
        .with_tx(|tx| {
            let nine = format!("{marker}-nine");
            Box::pin(async move {
                let count: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM app.repoexample WHERE string_value = $1"
                )
                .bind(nine)
                .fetch_one(&mut **tx)
                .await?;
                Ok(count)
            })
        })
        .await?;
    must_eq(hidden, 1);
    Ok(())
}

/// The company list is whatever grant is on the `&ctx` passed to the repo.
/// An open snapshot adds no `sys_client` predicate. A company list is one
/// `= ANY` predicate. An empty list matches nobody.
#[cfg_attr(miri, ignore)]
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn key_reads_the_grant_on_the_passed_context() -> Result<()> {
    let one = context_with_companies(Some(vec![9])).await?;
    let one_sql = format!("{:?}", RepoExample::update(&one, fixture::gen_update_row()));
    must_be_true(one_sql.contains("_x_.sys_client = "));
    must_eq(one_sql.matches("OR _x_.sys_client = ").count(), 0);

    let open = context_with_companies(None).await?;
    let open_sql = format!("{:?}", RepoExample::update(&open, fixture::gen_update_row()));
    must_be_true(!open_sql.contains("sys_client"));

    let nobody = context_with_companies(Some(vec![])).await?;
    let nobody_sql = format!("{:?}", RepoExample::patch(&nobody, fixture::gen_patches()));
    must_be_true(nobody_sql.contains("_x_.sys_client = ANY("));
    Ok(())
}
