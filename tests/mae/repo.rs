use crate::build::get_context;
use mae::repo;
use mae::request_context as mae_context;
use mae::request_context::ContextAccessor;
use std::sync::Arc;

#[derive(Clone)]
struct CustomContext;

type Context = Arc<mae_context::RequestContext<CustomContext>>;

#[repo::mae_repo("repoexample")]
pub struct RepoExample {
    pub value: i32,
    pub string_value: String,
}

impl repo::KeyAuths for RepoExample {
    fn keys() -> Vec<repo::builder::WhereCondition<Self>> {
        todo!()
    }
}

// #[test]
// fn should_make_domain_struct() {
//     let _my_repo = RepoExample {
//         value: 1,
//         string_value: String::from("hello_world"),
//         comment: None,
//         id: 1,
//         sys_client: 1,
//         status: DomainStatus::Active,
//         tags: SqlxJson::Array(vec![]),
//         sys_detail: SqlxJson::Object(Map::new()),
//         created_by: 1,
//         updated_by: 1,
//         updated_at: Utc::now(),
//         created_at: Utc::now(),
//     };
//     assert!(true);
// }
//
// #[tokio::test]
// async fn should_create_record() {
//     let ctx = get_context::<CustomContext>(CustomContext {})
//         .await
//         .unwrap();
//
//     let data = InsertRepoExample {
//         sys_client: 1,
//         status: DomainStatus::Active,
//         value: 1,
//         string_value: String::from("hello_world"),
//         comment: None,
//         tags: SqlxJson::Array(vec![]),
//         sys_detail: SqlxJson::Object(Map::new()),
//     };
//     let rec = RepoExample::insert(&ctx, data).await;
//     assert!(rec.is_ok());
//
//     assert_eq!(rec.unwrap().string_value, "hello_world");
// }
//
// #[tokio::test]
// async fn should_get_empty_records() {
//     let ctx = get_context::<CustomContext>(CustomContext {})
//         .await
//         .unwrap();
//
//     let builder = RepoExample::select_builder(1)
//         .unwrap()
//         .and_where(
//             RepoExampleFields::string_value,
//             Where::StringIs("".to_string()),
//         )
//         .and_where(RepoExampleFields::value, Where::Equals(1));
//
//     let res = builder.execute(&ctx).await;
//     assert!(res.is_ok());
//
//     assert!(res.unwrap().is_empty());
// }
//
// #[tokio::test]
// async fn should_get_records() {
//     let ctx = get_context::<CustomContext>(CustomContext {})
//         .await
//         .unwrap();
//
//     let builder = RepoExample::select_builder(1)
//         .unwrap()
//         .and_where(
//             RepoExampleFields::string_value,
//             Where::Ilike("%ELLO_WORL%".to_string()),
//         )
//         .and_where(RepoExampleFields::value, Where::Equals(1));
//
//     let res = builder.execute(&ctx).await;
//     assert!(res.is_ok());
//     assert_eq!(res.unwrap().is_empty(), false);
// }
//
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
