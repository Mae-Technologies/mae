use crate::common::context::Ctx;
use mae::repo::default::DomainStatus;
use mae::repo::filter::{Filter, FilterOp};
use mae::repo::implement::{KeyAuths, ToField};
use mae::repo::macros::schema;
pub use serde_json::Map;
use sqlx::Arguments;
pub use sqlx::types::JsonValue as SqlxJson;

#[schema(Ctx, "repoexample")]
#[allow(non_snake_case, non_camel_case_types, nonstandard_style)]
pub struct RepoExample {
    pub value: i32,
    pub string_value: String,
}

impl<F: ToField,> KeyAuths<F,> for RepoExample {
    fn keys() -> Vec<FilterOp<F,>,> {
        // TODO: This needs to actually add the rows.
        Vec::<FilterOp<F,>,>::new()
    }
}

// TODO: fixture methods should be dynamically generated randomly

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

pub fn gen_patches() -> Vec<PatchField,> {
    vec![
        PatchField::value(100,),
        PatchField::comment(Some("patching!".into(),),),
        PatchField::status(DomainStatus::Archived,),
    ]
}
pub fn gen_filters() -> Vec<FilterOp<Field,>,> {
    vec![FilterOp::Begin(Field::string_value, Filter::Like("sdfsdfsdfsd".into(),),)]
}
