use crate::build::get_context;
pub use chrono::Utc;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{Execute, Interface, KeyAuths, ToField};
use mae::repo::repo_macro::schema;
use mae::request_context as mae_context;
pub use serde_json::Map;
use sqlx::Arguments;
pub use sqlx::types::JsonValue as SqlxJson;
use std::sync::Arc;

#[derive(Clone)]
struct CustomContext;

type Context = mae_context::RequestContext<CustomContext>;

#[derive(Debug)]
#[schema("repoexample")]
pub struct RepoExample {
    pub value: i32,
    pub string_value: String,
}

impl<F: ToField> KeyAuths<F> for RepoExample {
    fn keys() -> Vec<FilterOp<F>> {
        // TODO: This needs to actually add the rows.
        Vec::<FilterOp<F>>::new()
    }
}

#[test]
fn should_make_domain_struct() {
    let _my_repo = RepoExample {
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
    assert!(true);
}

#[tokio::test]
async fn should_insert() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = Row {
        sys_client: Some(1),
        status: Some(DomainStatus::Active),
        value: Some(1),
        string_value: Some(String::from("hello_world")),
        comment: Some(None),
        tags: Some(SqlxJson::Array(vec![])),
        sys_detail: Some(SqlxJson::Object(Map::new())),
        id: None,
        // TODO: _by should be created dynamically with ctx, _at created dynamically with now()
        created_by: Some(1),
        updated_by: Some(1),
        updated_at: Some(Utc::now()),
        created_at: Some(Utc::now()),
    };
    // let data = RepoExample {
    // };
    let builder = RepoExample::insert_one(data);
    // println!("{}", builder);
    let rec = builder.fetch_all(&ctx).await;
    // println!("{:?}", rec);
    assert!(rec.is_ok());

    assert_eq!(rec.unwrap()[0].string_value, "hello_world");
}

#[tokio::test]
async fn should_get_empty_records() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let mut builder = RepoExample::select(vec![Field::value, Field::string_value]);

    builder = builder.filter(vec![
        FilterOp::Begin(Field::comment, Filter::Ilike("%bye-bye%".to_string())),
        FilterOp::Or(Field::string_value, Filter::Ilike("hello".to_string())),
    ]);

    // println!("{}", builder);
    let res = builder.fetch_all(&ctx).await;
    // println!("{:?}", res);
    assert!(res.is_ok());

    assert!(res.unwrap().is_empty());
}

#[tokio::test]
async fn should_get_records() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    //TODO: this should be refactored to a helper function to test on.
    let data = Row {
        sys_client: Some(1),
        status: Some(DomainStatus::Active),
        value: Some(1),
        string_value: Some(String::from("hello_world")),
        comment: Some(None),
        tags: Some(SqlxJson::Array(vec![])),
        sys_detail: Some(SqlxJson::Object(Map::new())),
        id: None,
        // TODO: _by should be created dynamically with ctx, _at created dynamically with now()
        created_by: Some(1),
        updated_by: Some(1),
        updated_at: Some(Utc::now()),
        created_at: Some(Utc::now()),
    };
    // let data = RepoExample {
    // };
    let builder = RepoExample::insert_one(data);
    // println!("{}", builder);
    let rec = builder.fetch_all(&ctx).await;
    // println!("{:?}", rec);
    assert!(rec.is_ok());

    assert_eq!(rec.unwrap()[0].string_value, "hello_world");

    let builder = RepoExample::select(vec![]).filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Ilike("%hello%".to_string()),
    )]);

    // println!("{}", builder);

    let res = builder.fetch_all(&ctx).await;
    // println!("{:?}", res);
    assert!(res.is_ok());
    assert_eq!(res.unwrap().is_empty(), false);
}

#[tokio::test]
async fn should_error_on_update_without_filters() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = Row {
        sys_client: Some(1),
        status: Some(DomainStatus::Deleted),
        value: Some(1),
        string_value: Some(String::from("updated_world")),
        comment: Some(None),
        tags: Some(SqlxJson::Array(vec![])),
        sys_detail: Some(SqlxJson::Object(Map::new())),
        id: None,
        // TODO: _by should be created dynamically with ctx, _at created dynamically with now()
        created_by: Some(1),
        updated_by: Some(1),
        updated_at: Some(Utc::now()),
        created_at: Some(Utc::now()),
    };
    let builder = RepoExample::update_many(data);

    let res = builder.fetch_all(&ctx).await;
    //
    assert!(res.is_err());
}

#[tokio::test]
async fn should_error_on_update_with_row_fields_all_none() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = Row {
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
    let mut builder = RepoExample::update_many(data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("hello_world".into()),
    )]);

    let res = builder.fetch_all(&ctx).await;
    //
    assert!(res.is_err());
}
#[tokio::test]
async fn should_update() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = Row {
        sys_client: Some(1),
        value: Some(1),
        status: None,
        string_value: Some(String::from("updated_world")),
        comment: Some(None),
        tags: Some(SqlxJson::Array(vec![])),
        sys_detail: Some(SqlxJson::Object(Map::new())),
        id: None,
        // TODO: _by should be created dynamically with ctx, _at created dynamically with now()
        created_by: Some(1),
        updated_by: Some(1),
        updated_at: Some(Utc::now()),
        created_at: Some(Utc::now()),
    };
    let mut builder = RepoExample::update_many(data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("hello_world".into()),
    )]);

    let res = builder.fetch_all(&ctx).await;
    //
    assert!(res.is_ok());
}

#[tokio::test]
async fn should_error_on_patch_without_filters() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = vec![
        PatchField::value(100),
        PatchField::comment(Some("patching!".into())),
        PatchField::status(DomainStatus::Archived),
    ];
    let mut builder = RepoExample::patch(data);

    let res = builder.fetch_all(&ctx).await;
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
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data: Vec<PatchField> = vec![];
    let mut builder = RepoExample::patch(data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("-- does not exist --".into()),
    )]);

    let res = builder.fetch_all(&ctx).await;
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
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = vec![
        PatchField::value(100),
        PatchField::comment(Some("patching!".into())),
        PatchField::status(DomainStatus::Archived),
    ];
    let mut builder = RepoExample::patch(data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("-- does not exist --".into()),
    )]);

    let res = builder.fetch_all(&ctx).await;
    //
    assert!(&res.is_ok());
    assert!(res.unwrap().len() == 0);
}
#[tokio::test]
async fn should_patch() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = vec![
        PatchField::value(100),
        PatchField::comment(Some("patching!".into())),
        PatchField::status(DomainStatus::Archived),
    ];
    let mut builder = RepoExample::patch(data);
    builder = builder.filter(vec![FilterOp::Begin(
        Field::string_value,
        Filter::Like("hello_world".into()),
    )]);

    let res = builder.fetch_all(&ctx).await;
    //
    assert!(res.is_ok());
}
