// TODO: delete this page
// use crate::request_context::ContextAccessor;
// use num::ToPrimitive;
// use sqlx::Arguments;
// use sqlx::postgres::PgArguments;
// use std::fmt;
//
// #[derive(Clone)]
// pub enum Where {
//     Equals(i32),
//     NotEquals(i32),
//     In(Vec<i32>),
//     NotIn(Vec<i32>),
//     Like(String),
//     NotLike(String),
//     Ilike(String),
//     NotIlike(String),
//     StringIs(String),
//     StringIsNot(String),
//     Gt(i32),
//     Gte(i32),
//     Lt(i32),
//     Lte(i32),
//     IsNull,
// }
// impl Where {
//     pub fn get_value(&self) -> ValueType {
//         match self {
//             Where::Equals(f) => ValueType::I32(f.clone()),
//             Where::NotEquals(f) => ValueType::I32(f.clone()),
//             Where::In(f) => ValueType::VecI32(f.clone()),
//             Where::NotIn(f) => ValueType::VecI32(f.clone()),
//             Where::Gt(f) => ValueType::I32(f.clone()),
//             Where::Gte(f) => ValueType::I32(f.clone()),
//             Where::Lt(f) => ValueType::I32(f.clone()),
//             Where::Lte(f) => ValueType::I32(f.clone()),
//             Where::Like(f) => ValueType::String(f.clone()),
//             Where::NotLike(f) => ValueType::String(f.clone()),
//             Where::Ilike(f) => ValueType::String(f.clone()),
//             Where::NotIlike(f) => ValueType::String(f.clone()),
//             Where::StringIs(f) => ValueType::String(f.clone()),
//             Where::StringIsNot(f) => ValueType::String(f.clone()),
//             Where::IsNull => ValueType::None,
//         }
//     }
// }
// impl fmt::Display for Where {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             Where::Equals(_) => write!(f, "="),
//             Where::NotEquals(_) => write!(f, "!="),
//             Where::In(_) => write!(f, "IN"),
//             Where::NotIn(_) => write!(f, "NOT IN"),
//             Where::Like(_) => write!(f, "LIKE"),
//             Where::NotLike(_) => write!(f, "NOT LIKE"),
//             Where::Ilike(_) => write!(f, "ILIKE"),
//             Where::NotIlike(_) => write!(f, "NOT ILIKE"),
//             Where::StringIs(_) => write!(f, "="),
//             Where::StringIsNot(_) => write!(f, "!="),
//             Where::Gt(_) => write!(f, ">"),
//             Where::Gte(_) => write!(f, ">="),
//             Where::Lt(_) => write!(f, "<"),
//             Where::Lte(_) => write!(f, "<="),
//             Where::IsNull => write!(f, "IS NULL"),
//         }
//     }
// }
//
// pub enum ValueType {
//     String(String),
//     VecI32(Vec<i32>),
//     I32(i32),
//     None,
// }
//
// #[derive(Clone)]
// pub enum WhereCondition<F: fmt::Display + Clone> {
//     And(F, Where),
//     Or(F, Where),
// }
// impl<F: fmt::Display + Clone> WhereCondition<F> {
//     pub fn get_value(&self) -> ValueType {
//         match self {
//             WhereCondition::Or(_, where_block) => where_block.get_value(),
//             WhereCondition::And(_, where_block) => where_block.get_value(),
//         }
//     }
// }
//
// impl<F: fmt::Display + Clone> fmt::Display for WhereCondition<F> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             WhereCondition::Or(field, where_block) => {
//                 write!(
//                     f,
//                     "{}",
//                     format!("OR {} {}", field.to_string(), where_block.to_string())
//                 )
//             }
//             WhereCondition::And(field, where_block) => {
//                 write!(
//                     f,
//                     "{}",
//                     format!("AND {} {}", field.to_string(), where_block.to_string())
//                 )
//             }
//         }
//     }
// }
//
// #[derive(Clone)]
// pub struct WhereBlock<F: Clone + fmt::Display>(pub Vec<WhereCondition<F>>);
//
// #[derive(Clone)]
// pub struct UpdateRepo<UF, F: Clone + fmt::Display> {
//     pub update_block: Vec<UF>,
//     pub where_block: WhereBlock<F>,
// }
//
// #[derive(Clone)]
// pub struct SelectRepo<F: Clone + fmt::Display> {
//     pub where_block: WhereBlock<F>,
//     pub build_string: Option<String>,
// }
//
// pub trait UpdateBuilder<
//     UF: fmt::Display + BindTo,
//     F: Clone + fmt::Display,
//     R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
//     C: ContextAccessor + Clone + Sync,
// >: WhereBuilder<F>
// {
//     fn update_builder(
//         fields: Vec<UF>,
//         sys_client: u64,
//     ) -> Result<UpdateRepo<UF, F>, anyhow::Error> {
//         let where_block = WhereBlock(vec![WhereCondition::And(
//             Self::get_sys_client_field(),
//             Where::Equals(
//                 sys_client
//                     .to_i32()
//                     .ok_or_else(|| anyhow::anyhow!("unable to convert sys_client to i32."))?,
//             ),
//         )]);
//         Ok(UpdateRepo::<UF, F> {
//             update_block: fields,
//             where_block,
//         })
//     }
//     fn get_repo_name() -> String;
//     fn execute(
//         &self,
//         ctx: &C,
//     ) -> impl std::future::Future<Output = Result<Vec<R>, anyhow::Error>> + Send
//     where
//         Self: Sync,
//     {
//         async move {
//             let mut args = PgArguments::default();
//
//             for update_f in self.get_update_block().iter() {
//                 update_f.bind(&mut args);
//             }
//             for where_cond in self.get_where_block().0.iter() {
//                 let value_type = where_cond.get_value();
//                 let _ = match value_type {
//                     ValueType::String(v) => args.add(v),
//                     ValueType::VecI32(v) => args.add(v),
//                     ValueType::I32(v) => args.add(v),
//                     ValueType::None => Ok(()),
//                 };
//             }
//             let build_string = &self.build_string();
//             let query = sqlx::query_as_with(&build_string, args);
//             Ok(query.fetch_all(ctx.db_pool()).await?)
//         }
//     }
//     fn get_update_block(&self) -> &Vec<UF>;
//     fn get_update_string(&self) -> String {
//         let mut update_fields: Vec<String> = vec![];
//         for (i, field) in self.get_update_block().iter().enumerate() {
//             let field = field.to_string().split("(").next().unwrap().to_string();
//             // the first where condition has an 'and ' at the begining of the string. this
//             // needs to be removed
//             if i == 0 {
//                 update_fields.push(format!("{} = ${}", field.clone(), i + 1));
//             } else {
//                 update_fields.push(format!("{} = ${}", field.clone(), i + 1));
//             }
//         }
//         update_fields.join(" ")
//     }
//     fn build_string(&self) -> String {
//         let where_string = self.get_where_string(Some(self.get_update_block().len()));
//         let update_fields = self.get_update_string();
//         let sql_string = format!(
//             "UPDATE {} SET {} WHERE {} RETURNING *;",
//             Self::get_repo_name(),
//             update_fields,
//             where_string
//         );
//         sql_string
//     }
//     fn get_sys_client_field() -> F;
// }
//
// pub trait SelectBuilder<
//     F: Clone + fmt::Display,
//     R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
//     C: ContextAccessor + Clone + Sync,
// >: WhereBuilder<F>
// {
//     fn select_builder(sys_client: u64) -> Result<SelectRepo<F>, anyhow::Error> {
//         let where_block = WhereBlock(vec![WhereCondition::And(
//             Self::get_sys_client_field(),
//             Where::Equals(
//                 sys_client
//                     .to_i32()
//                     .ok_or_else(|| anyhow::anyhow!("unable to convert sys_client to i32."))?,
//             ),
//         )]);
//         Ok(SelectRepo::<F> {
//             where_block,
//             build_string: None,
//         })
//     }
//     fn get_repo_name() -> String;
//     fn execute(
//         &self,
//         ctx: &C,
//     ) -> impl std::future::Future<Output = Result<Vec<R>, anyhow::Error>> + Send
//     where
//         Self: Sync,
//     {
//         async move {
//             let build_string = &self.build_string();
//             let mut query = sqlx::query_as::<sqlx::Postgres, R>(&build_string);
//             for where_cond in self.get_where_block().0.iter() {
//                 let value_type = where_cond.get_value();
//                 match value_type {
//                     ValueType::String(v) => query = query.bind(v),
//                     ValueType::VecI32(v) => query = query.bind(v),
//                     ValueType::I32(v) => query = query.bind(v),
//                     ValueType::None => {}
//                 };
//             }
//             Ok(query.fetch_all(ctx.db_pool()).await?)
//         }
//     }
//     fn build_string(&self) -> String {
//         let where_string = self.get_where_string(None);
//         let sql_string = format!(
//             "SELECT * FROM {} WHERE {};",
//             Self::get_repo_name(),
//             where_string
//         );
//         sql_string
//     }
//     fn get_sys_client_field() -> F;
// }
//
// pub trait WhereBuilder<F: Clone + fmt::Display> {
//     fn get_where_block(&self) -> WhereBlock<F>;
//
//     fn or_where(&self, field: F, where_block: Where) -> Self
//     where
//         Self: Sized,
//     {
//         let mut where_blocks = self.get_where_block().0.clone();
//         where_blocks.push(WhereCondition::<F>::Or(field, where_block));
//         self.copy_with(WhereBlock::<F>(where_blocks))
//     }
//
//     fn and_where(&self, field: F, where_block: Where) -> Self
//     where
//         Self: Sized,
//     {
//         let mut where_blocks = self.get_where_block().0.clone();
//         where_blocks.push(WhereCondition::<F>::And(field, where_block));
//         self.copy_with(WhereBlock::<F>(where_blocks))
//     }
//
//     fn copy_with(&self, where_block: WhereBlock<F>) -> Self;
//     fn get_where_string(&self, binding_offset_option: Option<usize>) -> String {
//         let mut where_conds: Vec<String> = vec![];
//         let offset = match binding_offset_option {
//             Some(v) => v,
//             None => 0,
//         };
//         for (i, field) in self.get_where_block().0.iter().enumerate() {
//             // the first where condition has an 'and ' at the begining of the string. this
//             // needs to be removed
//             if i == 0 {
//                 let field = &field.to_string()[4..].to_owned();
//                 where_conds.push(format!("{} ${}", field, i + 1 + offset));
//             } else {
//                 where_conds.push(format!(
//                     "{} ${}",
//                     field.to_string().as_str().to_owned(),
//                     i + 1 + offset
//                 ));
//             }
//         }
//         where_conds.join(" ")
//     }
// }
//
// pub trait BindTo {
//     fn bind(&self, args: &mut sqlx::postgres::PgArguments);
// }
