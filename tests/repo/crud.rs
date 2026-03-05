use crate::common::context::get_context;
use crate::common::must::{Must, MustExpect, must_be_true, must_eq};
use crate::repo::fixture::Field;
use crate::repo::fixture::{self, RepoExample};
use anyhow::Result;
use chrono::Utc;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{Execute, Interface};
use mae::request_context::ContextAccessor;
use mae_macros::mae_test;
use serde_json::Map;
use sqlx::types::JsonValue as SqlxJson;

#[mae_test(not_async)]
fn should_make_domain_struct() {
    let _my_repo = fixture::RepoExample {
        value: 1,
        string_value: String::from("hello_world",),
        comment: None,
        id: 1,
        sys_client: 1,
        status: DomainStatus::Active,
        tags: SqlxJson::Array(vec![],),
        sys_detail: SqlxJson::Object(Map::new(),),
        created_by: 1,
        updated_by: 1,
        updated_at: Utc::now(),
        created_at: Utc::now(),
    };
}

#[mae_test]
async fn should_insert() -> Result<(),> {
    let ctx = get_context().await.must();

    let data = fixture::gen_row(); // let data = RepoExample {
    // };
    let builder = fixture::RepoExample::insert_one(&ctx, data,);
    // println!("{}", builder);
    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await.must();
    // println!("{:?}", res);

    must_eq(res[0].string_value.as_str(), "hello_world",);
    Ok((),)
}

#[mae_test]
async fn should_get_empty_records() -> Result<(),> {
    let ctx = get_context().await.must();

    let mut builder = fixture::RepoExample::select(&ctx, vec![Field::All],);

    builder = builder.filter(vec![
        FilterOp::Begin(Field::comment, Filter::Ilike("%bye-bye%".to_string(),),),
        FilterOp::Or(Field::string_value, Filter::Ilike("hello".to_string(),),),
    ],);

    // println!("{}", builder);
    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await.must();
    // println!("{:?}", res);

    must_be_true(res.is_empty(),);
    Ok((),)
}

#[mae_test]
async fn should_get_records() -> Result<(),> {
    let ctx = get_context().await.must();
    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);

    let data = fixture::gen_row();

    let builder = fixture::RepoExample::insert_one(&ctx, data,);
    // println!("{}", builder);
    let res = builder.fetch_all(&mut conn,).await.must();
    // println!("{:?}", rec);

    must_eq(res[0].string_value.as_str(), "hello_world",);

    let builder = fixture::RepoExample::select(&ctx, vec![Field::All],);
    // Vjfilter(vec![FilterOp::Begin(
    //     Field::string_value,
    //     Filter::Ilike("%hello%".to_string()),
    // )]);

    println!("{}", builder);

    let res = builder.fetch_all(&mut conn,).await.must();
    println!("{:?}", res);

    must_be_true(!res.is_empty(),);
    Ok((),)
}

#[mae_test]
async fn should_error_on_update_without_filters() -> Result<(),> {
    let ctx = get_context().await.must();

    let data = fixture::gen_update_row();
    let builder = fixture::RepoExample::update_many(&ctx, data,);

    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await;
    //
    res.err().must();
    Ok((),)
}

#[mae_test]
async fn should_error_on_update_with_row_fields_all_none() -> Result<(),> {
    let ctx = get_context().await.must();

    // UpdateRow: all fields None => no bindings => should error
    let data = fixture::UpdateRow {
        status: None,
        value: None,
        string_value: None,
        comment: None,
        tags: None,
        sys_detail: None,
    };
    let mut builder = fixture::RepoExample::update_many(&ctx, data,);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into(),),)],);

    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await;
    //
    res.err().must();
    Ok((),)
}
#[mae_test]
async fn should_update() -> Result<(),> {
    let ctx = get_context().await.must();

    let new_data = fixture::gen_row();

    let _ = RepoExample::insert_one(&ctx, new_data,).fetch_all(ctx.db_pool(),).await;

    let data = fixture::gen_update_row();
    let mut builder = fixture::RepoExample::update_many(&ctx, data,);
    builder = builder
        .filter(vec![FilterOp::Begin(Field::string_value, Filter::Like("hello_world".into(),),)],);

    // println!("{}", builder);
    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let _res = builder.fetch_all(&mut conn,).await.must();

    Ok((),)
}

#[mae_test]
async fn should_error_on_patch_without_filters() -> Result<(),> {
    let ctx = get_context().await.must();

    let data = fixture::gen_patches();
    let builder = fixture::RepoExample::patch(&ctx, data,);

    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await;
    //
    must_be_true(res.err().must().to_string().contains("Unable to Update/Patch",),);
    Ok((),)
}
#[mae_test]
async fn should_error_on_patch_with_fields_empty() -> Result<(),> {
    let ctx = get_context().await.must();

    let data: Vec<fixture::PatchField,> = vec![];
    let mut builder = fixture::RepoExample::patch(&ctx, data,);
    builder = builder.filter(fixture::gen_filters(),);

    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await;
    //
    must_be_true(res.is_err(),);
    must_be_true(res.err().must().to_string().contains("Unable to Update/Patch",),);
    Ok((),)
}
#[mae_test]
async fn patch_should_return_empty() -> Result<(),> {
    let ctx = get_context().await.must();

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data,);
    builder = builder.filter(fixture::gen_filters(),);

    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let res = builder.fetch_all(&mut conn,).await.must();
    //
    must_be_true(res.is_empty(),);
    Ok((),)
}
#[mae_test]
async fn should_patch() -> Result<(),> {
    let ctx = get_context().await.must();

    let data = fixture::gen_patches();
    let mut builder = fixture::RepoExample::patch(&ctx, data,);
    builder = builder.filter(fixture::gen_filters(),);

    // println!("{}", builder);
    let mut conn =
        ctx.custom.scoped_connection().await.must_expect("failed to get db connection.",);
    let _res = builder.fetch_all(&mut conn,).await.must();
    // println!("{:?}", res);

    Ok((),)
}
