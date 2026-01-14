use crate::build::get_context;
pub use chrono::Utc;
use mae::repo;
use mae::repo::builder::{Filter, Interface, Where, WhereCondition};
use mae::request_context as mae_context;
pub use serde_json::Map;
use sqlx::Arguments;
pub use sqlx::types::JsonValue as SqlxJson;
use std::sync::Arc;

#[derive(Clone)]
struct CustomContext;

type Context = Arc<mae_context::RequestContext<CustomContext>>;

#[repo::mae_repo("repoexample")]
pub struct RepoExample {
    pub value: i32,
    pub string_value: String,
}

impl<F: mae::repo::builder::Filter> mae::repo::builder::KeyAuths<F> for RepoExample {
    fn keys() -> Vec<repo::builder::WhereCondition<F>> {
        // TODO: This needs to actually add the rows.
        Vec::<repo::builder::WhereCondition<F>>::new()
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
        status: repo::fields::DomainStatus::Active,
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
async fn should_create_record() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let data = _Row::Options(_OptionRow {
        sys_client: Some(1),
        status: Some(repo::fields::DomainStatus::Active),
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
    });
    // let data = RepoExample {
    // };
    let builder = RepoExample::insert_many(vec![data]);
    println!("{}", builder);
    let rec = builder.fetch_all(ctx).await;
    println!("{:?}", rec);
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
        WhereCondition::Begin(Field::comment, Where::Ilike("%bye-bye%".to_string())),
        WhereCondition::Or(Field::string_value, Where::Ilike("hello".to_string())),
    ]);

    println!("{}", builder);
    let res = builder.fetch_all(ctx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    assert!(res.unwrap().is_empty());
}

#[tokio::test]
async fn should_get_records() {
    let ctx = get_context::<CustomContext>(CustomContext {})
        .await
        .unwrap();

    let builder = RepoExample::select(vec![]).filter(vec![WhereCondition::Begin(
        Field::string_value,
        Where::Ilike("%hello%".to_string()),
    )]);

    println!("{}", builder);

    let res = builder.fetch_all(ctx).await;
    println!("{:?}", res);
    assert!(res.is_ok());
    assert_eq!(res.unwrap().is_empty(), false);
}

// #[tokio::test]
// async fn should_update_records() {
//     let ctx = get_context::<CustomContext>(CustomContext {})
//         .await
//         .unwrap();
//
//     let builder = RepoExample::update_builder(vec![RepoExampleUpdateFields::value(11)], 1)
//         .unwrap()
//         .and_where(RepoExampleFields::value, Where::Equals(1));
//
//     println!("{}", builder.build_string());
//
//     let res = builder.execute(&ctx).await;
//     println!("{:?}", res);
//     //
//     assert!(res.is_ok());
// }
