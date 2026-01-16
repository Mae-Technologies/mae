use crate::common::context::{self, get_context};
use crate::repo::fixture::Field;
use crate::repo::fixture::{self, RepoExample};
pub use chrono::Utc;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{Execute, Interface};
use mae::request_context::ContextAccessor;
pub use serde_json::Map;
pub use sqlx::types::JsonValue as SqlxJson;

#[test]
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
        created_at: Utc::now(),
    };
}

#[tokio::test]
async fn should_insert() {
    let ctx = get_context().await.unwrap();

    let data = fixture::gen_row(); // let data = RepoExample {
    // };
    let builder = fixture::RepoExample::insert_one(&ctx, data);
    // println!("{}", builder);
    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    assert_eq!(res.unwrap()[0].string_value, "hello_world");
}

#[tokio::test]
async fn should_get_empty_records() {
    let ctx = get_context().await.unwrap();

    let mut builder = fixture::RepoExample::select(&ctx, vec![Field::All]);

    builder = builder.filter(vec![
        FilterOp::Begin(Field::comment, Filter::Ilike("%bye-bye%".to_string())),
        FilterOp::Or(Field::string_value, Filter::Ilike("hello".to_string())),
    ]);

    //println!("{}", builder);
    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    // println!("{:?}", res);
    assert!(res.is_ok());

    assert!(res.unwrap().is_empty());
}

#[tokio::test]
async fn should_get_records() {
    let ctx = get_context().await.unwrap();
    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");

    let data = fixture::gen_row();

    let builder = fixture::RepoExample::insert_one(&ctx, data);
    // println!("{}", builder);
    let rec = builder.fetch_all(&mut conn).await;
    println!("{:?}", rec);
    assert!(rec.is_ok());

    assert_eq!(rec.unwrap()[0].string_value, "hello_world");

    let builder = fixture::RepoExample::select(&ctx, vec![Field::All]);
    // Vjfilter(vec![FilterOp::Begin(
    //     Field::string_value,
    //     Filter::Ilike("%hello%".to_string()),
    // )]);

    println!("{}", builder);

    let res = builder.fetch_all(&mut conn).await;
    println!("{:?}", res);
    assert!(res.is_ok());
    assert!(!res.unwrap().is_empty());
}

#[tokio::test]
async fn should_error_on_update_without_filters() {
    let ctx = get_context().await.unwrap();

    let data = fixture::gen_row();
    let builder = fixture::RepoExample::update_many(&ctx, data);

    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    //
    assert!(res.is_err());
}

#[tokio::test]
async fn should_error_on_update_with_row_fields_all_none() {
    let ctx = get_context().await.unwrap();

    let data = fixture::Row {
        sys_client: None,
        value: None,
        status: None,
        string_value: None,
        comment: None,
        tags: None,
        sys_detail: None,
        id: None,
        // TODO: _by should be created dynamically with ctx, _at created dynamically with now()
        created_by: None,
        updated_by: None,
        updated_at: None,
        created_at: None,
    };
    let mut builder = fixture::RepoExample::update_many(&ctx, data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("hello_world".into()),
    )]);

    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    //
    assert!(res.is_err());
}
#[tokio::test]
async fn should_update() {
    let ctx = get_context().await.unwrap();

    let new_data = fixture::gen_row();

    let _ = RepoExample::insert_one(&ctx, new_data)
        .fetch_all(ctx.db_pool())
        .await;

    let data = fixture::gen_row();
    let mut builder = fixture::RepoExample::update_many(&ctx, data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("hello_world".into()),
    )]);

    // println!("{}", builder);
    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    assert!(res.is_ok());

    context::shutdown_testcontainers().await;
}

#[tokio::test]
async fn should_error_on_patch_without_filters() {
    let ctx = get_context().await.unwrap();

    let data = fixture::gen_patches();
    let builder = fixture::RepoExample::patch(&ctx, data);

    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    //
    assert!(&res.is_err());
    assert!(
        res.err()
            .unwrap()
            .to_string()
            .contains("Unable to Update/Patch")
    );
}
#[tokio::test]
async fn should_error_on_patch_with_fields_empty() {
    let ctx = get_context().await.unwrap();

    let data: Vec<fixture::PatchField> = vec![];
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    //
    assert!(&res.is_err());
    assert!(
        res.err()
            .unwrap()
            .to_string()
            .contains("Unable to Update/Patch")
    );
}
#[tokio::test]
async fn patch_should_return_empty() {
    let ctx = get_context().await.unwrap();

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    //
    assert!(&res.is_ok());
    assert!(res.unwrap().is_empty());
}
#[tokio::test]
async fn should_patch() {
    let ctx = get_context().await.unwrap();

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data);
    builder = builder.filter(fixture::gen_filters());

    // println!("{}", builder);
    let mut conn = ctx
        .custom
        .scoped_connection()
        .await
        .expect("failed to get db connection.");
    let res = builder.fetch_all(&mut conn).await;
    // println!("{:?}", res);
    assert!(res.is_ok());
}
