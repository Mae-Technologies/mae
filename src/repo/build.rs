use anyhow::{Result, anyhow};
use std::fmt::Display;
use std::marker::PhantomData;

use sqlx::{Executor, Postgres};

use super::map_util::{BindArgs, FilterOp, SqlStatement, concat_sql_parts, sql_where};
use super::type_def::{Context, QueryAs, ToField, ToPatch, ToRow};

// /////
// INTERFACE TO THE SCHEMA def
//  ////

pub trait Build<C: Context, R: ToRow, U: ToRow, F: ToField, P: ToPatch,>:
    QueryAs + KeyAuths<F,>
{
    // Get a builder so we can build some SQL
    fn build_insert(statement: SqlStatement<R, U, F, P,>,) -> Builder<C, Self, R, U, F, P,> {
        Builder::<C, Self, R, U, F, P,> {
            statement,
            filters: Self::keys(),
            schema: Self::schema(),
            ctx: PhantomData,
            query_as: PhantomData,
            returning: None,
        }
    }
    // Extend a method so we can access the SQL Schema
    fn schema() -> String;
}

// expose the auth keys for auth/auth options
// TODO: migrate this trait KeyAuths, to a custom set of functions that return a fn() for multiple
// circumstances:
//      - What to prevent Selects Updates & Inserts on based on context
//      - What to Filter on based on context
pub trait KeyAuths<F: ToField,> {
    fn keys() -> Vec<FilterOp<F,>,>;
}

// Expose methods to the user defined struct
// _ctx in the methods is for a future feature
// TODO: impl ctx
pub trait Interface<C: Context, R: ToRow, U: ToRow, F: ToField, P: ToPatch,>:
    Build<C, R, U, F, P,>
{
    fn insert_one(_ctx: &C, rec: R,) -> Builder<C, Self, R, U, F, P,> {
        Self::build_insert(SqlStatement::<R, U, F, P,>::InsertOne(rec,),)
    }

    fn insert_many(_ctx: &C, recs: Vec<R,>,) -> Builder<C, Self, R, U, F, P,> {
        Self::build_insert(SqlStatement::<R, U, F, P,>::InsertMany(recs,),)
    }

    fn select(_ctx: &C, rec: Vec<F,>,) -> Builder<C, Self, R, U, F, P,> {
        Self::build_insert(SqlStatement::<R, U, F, P,>::Select(rec,),)
    }
    fn update_many(_ctx: &C, rec: U,) -> Builder<C, Self, R, U, F, P,> {
        Self::build_insert(SqlStatement::Update(rec,),)
    }
    fn patch(_ctx: &C, recs: Vec<P,>,) -> Builder<C, Self, R, U, F, P,> {
        Self::build_insert(SqlStatement::Patch(recs,),)
    }
}

// Anything that implements Build `B` implements the Interface
impl<C: Context, R: ToRow, U: ToRow, F: ToField, P: ToPatch, B: Build<C, R, U, F, P,>,>
    Interface<C, R, U, F, P,> for B
{
}

// ////
// BUILDER MECHANICS
// ////

// Our builder will be composed of multiple parts
pub struct Builder<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,> {
    // SELECT, UPDATE, INSERT ...
    statement: SqlStatement<R, U, F, P,>,
    // WHERE ...
    filters: Vec<FilterOp<F,>,>,
    // Schema
    schema: String,
    // Context, required for sql executions
    ctx: PhantomData<fn() -> C,>,
    // QueryAs, required for sql executions
    query_as: PhantomData<fn() -> A,>,
    //  what are we returning? (Insert / Update)
    returning: Option<Vec<F,>,>,
    // TODO: Add offset/limit's
}

// expose build methods with the *Builder Pattern*
impl<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,>
    Builder<C, A, R, U, F, P,>
{
    // add filters to the query
    pub fn filter(mut self, mut values: Vec<FilterOp<F,>,>,) -> Self {
        self.filters.append(&mut values,);
        self
    }
    pub fn returning(mut self, values: Vec<F,>,) -> Self {
        self.returning = Some(values,);
        self
    }
    // add offset to the query
    // TODO: implement these
    // pub fn offset(mut self, offset: usize) -> Self {
    //     todo!()
    // }
    // // add limit to the query
    // pub fn limit(mut self, limit: usize) -> Self {
    //     todo!()
    // }
}

// The builder has to convert-to and execute SQL Queries:
//

// The Builder can convert to SQL Queries
impl<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,> ToSql<R, U, F, P,>
    for Builder<C, A, R, U, F, P,>
{
    fn statement(&self,) -> &SqlStatement<R, U, F, P,> {
        &self.statement
    }
    fn schema(&self,) -> &String {
        &self.schema
    }
    fn filters(&self,) -> &Vec<FilterOp<F,>,> {
        &self.filters
    }
}

// The Builder can execute SQL Queries
// Self is required to be ToSql so that it can access it's conversion methods
impl<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,> Execute<C, A, R, U, F, P,>
    for Builder<C, A, R, U, F, P,>
where
    Self: ToSql<R, U, F, P,>,
{
}

// Turn the type into an SQL Statement, from parts
pub trait ToSql<R: ToRow, U: ToRow, F: ToField, P: ToPatch,> {
    // get the INSERT, SELECT, UPDATE method
    fn statement(&self,) -> &SqlStatement<R, U, F, P,>;
    // get the WHERE method
    fn filters(&self,) -> &Vec<FilterOp<F,>,>;
    // get the schema name
    fn schema(&self,) -> &String;
    // TODO: downstream checks on uniqueness for CRUD Operations
    fn to_sql(&self,) -> Result<String,> {
        // TODO: The Returning cmd is hard coded for now. There has to be a new impl to get all fields.
        let where_str = sql_where(self.filters(), self.statement().bind_len(), None,);
        Ok(match &self.statement() {
            SqlStatement::Select(field_blocks,) => {
                let fields = field_blocks
                    .iter()
                    .map(|field| -> Result<String,> {
                        let (mut str_value, _,) = field.to_sql_parts();
                        str_value.pop().ok_or_else(|| anyhow!("cannot find binding index"),)
                    },)
                    .collect::<Result<Vec<_,>,>>()?
                    .join(", ",);
                format!("SELECT {} FROM {}{};", &fields, self.schema(), where_str,)
            }
            SqlStatement::InsertOne(row,) => {
                let (fields, bind_idx,) = row.to_sql_parts();
                let fields_str = fields.join(", ",);
                let bind_idx_str =
                    bind_idx.ok_or_else(|| anyhow!("cannot find binding index"),)?.join(", ",);
                format!(
                    "INSERT INTO {} ({}) VALUES ({}){} RETURNING *;",
                    self.schema(),
                    fields_str,
                    bind_idx_str,
                    where_str,
                )
            }

            SqlStatement::InsertMany(_,) => {
                unimplemented!();
            }
            SqlStatement::Update(row,) => {
                // TODO: this is with a Row, meaning it has an ID, the Where Statement will be
                // appended with id = x
                let where_str =
                    sql_where(self.filters(), self.statement().bind_len(), Some("_x_".into(),),);
                let (fields, bind_idx,) = row.to_sql_parts();
                let f = fields.join(", ",);
                let f_f = fields
                    .into_iter()
                    .map(|f| format!("{f} = _z_.{f}"),)
                    .collect::<Vec<_,>>()
                    .join(", ",);
                let v = bind_idx.into_iter().flatten().collect::<Vec<_,>>().join(", ",);
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_ SET {f_f} FROM (VALUES ({v})) AS _z_({f}) {where_str} RETURNING *;"
                )
            }
            SqlStatement::Patch(fields,) => {
                let where_str =
                    sql_where(self.filters(), self.statement().bind_len(), Some("_x_".into(),),);
                // NOTE: the binding could not be calculated at that level. It has to be done
                // manually
                let (fields, _,) = concat_sql_parts(
                    fields.iter().map(|f| f.to_sql_parts(),).collect::<Vec<_,>>(),
                );
                let bind_idx = 0..fields.len();
                let f = fields.join(", ",);
                let f_f = fields
                    .iter()
                    .map(|f| format!("{f} = _z_.{f}"),)
                    .collect::<Vec<_,>>()
                    .join(", ",);
                let v = bind_idx.map(|i| format!("${:?}", i + 1),).collect::<Vec<_,>>().join(", ",);
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_ SET {f_f} FROM (VALUES ({v})) AS _z_({f}) {where_str} RETURNING *;"
                )
            }
        },)
    }
    // bind the arguments to the SQL query
    fn args(&self,) -> sqlx::postgres::PgArguments {
        let mut args = sqlx::postgres::PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.statement().bind(&mut args,);
        for w in self.filters().iter() {
            w.bind(&mut args,);
        }
        args
    }
}

// Expose Execution methods
// Self has to impl ToSql so that it can access SQL Conversion Methods
pub trait Execute<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,>:
    ToSql<R, U, F, P,>
{
    fn execute(&self, _ctx: &C,) -> impl std::future::Future<Output = anyhow::Result<(),>,> + Send
    where
        Self: Sync,
    {
        async { todo!() }
    }
    fn fetch_optional(
        &self,
        _ctx: &C,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<A,>,>,> + Send
    where
        Self: Sync,
    {
        async { todo!() }
    }
    fn fetch_one(&self, _ctx: &C,) -> impl std::future::Future<Output = anyhow::Result<A,>,> + Send
    where
        Self: Sync,
    {
        async { todo!() }
    }
    fn fetch_all<'c,>(
        &self,
        exec: impl Executor<'c, Database = Postgres,>,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<A,>,>,> + Send
    where
        Self: Sync,
    {
        async move {
            match self.statement() {
                SqlStatement::Update(_,) | SqlStatement::Patch(_,) => {
                    if self.filters().is_empty() {
                        return Err(anyhow!("Unable to Update/Patch without filters"),);
                    }
                    if self.statement().bind_len() < 1 {
                        return Err(anyhow!("Unable to Update/Patch with empty fields"),);
                    }
                }
                SqlStatement::Select(field_blocks,) => {
                    if field_blocks.len() != 1 {
                        return Err(anyhow!(
                            "Unable to use the fetch_all method while choosing which fields to return. Use the fetch_all_raw() method."
                        ),);
                    }
                }
                _ => {}
            }
            let sql = self.to_sql()?;
            let req = sqlx::query_as_with::<'_, sqlx::Postgres, A, sqlx::postgres::PgArguments,>(
                &sql,
                self.args(),
            );
            let res: anyhow::Result<Vec<A,>,> = req
                .fetch_all(exec,)
                .await
                .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e),);
            res
        }
    }
}

// Display the SQL to the user.
impl<C: Context, A: QueryAs, R: ToRow, U: ToRow, F: ToField, P: ToPatch,> Display
    for Builder<C, A, R, U, F, P,>
where
    Self: ToSql<R, U, F, P,>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_,>,) -> std::fmt::Result {
        write!(f, "{}", self.to_sql().map_err(|_| std::fmt::Error)?)
    }
}
