use std::fmt::Display;
use std::marker::PhantomData;

use super::map_util::{BindArgs, FilterOp, SqlStatement, ToSqlParts, concat_sql_parts, sql_where};
use super::type_def::{Context, QueryAs, ToField, ToPatch, ToRow};
use crate::request_context::ContextAccessor;

// /////
// INTERFACE TO THE SCHEMA def
//  ////

pub trait Build<C: Context, R: ToRow, F: ToField, P: ToPatch>: QueryAs + KeyAuths<F> {
    // Get a builder so we can build some SQL
    fn build(statement: SqlStatement<R, F, P>) -> Builder<C, Self, R, F, P> {
        Builder::<C, Self, R, F, P> {
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
pub trait KeyAuths<F: ToField> {
    fn keys() -> Vec<FilterOp<F>>;
}

// Expose methods to the user defined struct
pub trait Interface<C: Context, R: ToRow, F: ToField, P: ToPatch>: Build<C, R, F, P> {
    fn insert_one(rec: R) -> Builder<C, Self, R, F, P> {
        Self::build(SqlStatement::<R, F, P>::InsertOne(rec))
    }

    fn insert_many(recs: Vec<R>) -> Builder<C, Self, R, F, P> {
        Self::build(SqlStatement::<R, F, P>::InsertMany(recs))
    }

    fn select(rec: Vec<F>) -> Builder<C, Self, R, F, P> {
        Self::build(SqlStatement::<R, F, P>::Select(rec))
    }
    fn update_many(rec: R) -> Builder<C, Self, R, F, P> {
        Self::build(SqlStatement::Update(rec))
    }
    fn patch(recs: Vec<P>) -> Builder<C, Self, R, F, P> {
        Self::build(SqlStatement::Patch(recs))
    }
}

// Anything that implements Build `B` implements the Interface
impl<C: Context, R: ToRow, F: ToField, P: ToPatch, B: Build<C, R, F, P>> Interface<C, R, F, P>
    for B
{
}

// ////
// BUILDER MECHANICS
// ////

// Our builder will be composed of multiple parts
pub struct Builder<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch> {
    // SELECT, UPDATE, INSERT ...
    statement: SqlStatement<R, F, P>,
    // WHERE ...
    filters: Vec<FilterOp<F>>,
    // Schema
    schema: String,
    // Context, required for sql executions
    ctx: PhantomData<fn() -> C>,
    // QueryAs, required for sql executions
    query_as: PhantomData<fn() -> A>,
    //  what are we returning? (Insert / Update)
    returning: Option<Vec<F>>,
    // TODO: Add offset/limit's
}

// expose build methods with the *Builder Pattern*
impl<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch> Builder<C, A, R, F, P> {
    // add filters to the query
    pub fn filter(mut self, mut values: Vec<FilterOp<F>>) -> Self {
        self.filters.append(&mut values);
        self
    }
    pub fn returning(mut self, mut values: Vec<F>) -> Self {
        self.returning = Some(values);
        self
    }
    // add offset to the query
    pub fn offset(mut self, offset: usize) -> Self {
        todo!()
    }
    // add limit to the query
    pub fn limit(mut self, limit: usize) -> Self {
        todo!()
    }
}

//
// The builder has to convert-to and execute SQL Queries:
//

// The Builder can convert to SQL Queries
impl<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch> ToSql<R, F, P>
    for Builder<C, A, R, F, P>
{
    fn statement(&self) -> &SqlStatement<R, F, P> {
        &self.statement
    }
    fn schema(&self) -> &String {
        &self.schema
    }
    fn filters(&self) -> &Vec<FilterOp<F>> {
        &self.filters
    }
}

// The Builder can execute SQL Queries
// Self is required to be ToSql so that it can access it's conversion methods
impl<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch> Execute<C, A, R, F, P>
    for Builder<C, A, R, F, P>
where
    Self: ToSql<R, F, P>,
{
}

// Turn the type into an SQL Statement, from parts
pub trait ToSql<R: ToRow, F: ToField, P: ToPatch> {
    // get the INSERT, SELECT, UPDATE method
    fn statement(&self) -> &SqlStatement<R, F, P>;
    // get the WHERE method
    fn filters(&self) -> &Vec<FilterOp<F>>;
    // get the schema name
    fn schema(&self) -> &String;
    fn to_sql(&self) -> String {
        // TODO: The Returning cmd is fixed for now. There has to be a new impl to get all fields.
        let where_str = sql_where(&self.filters(), self.statement().bind_len(), None);
        match &self.statement() {
            SqlStatement::Select(field_blocks) => {
                let fields = field_blocks
                    .iter()
                    .map(|field| {
                        let (mut str_value, _) = field.to_sql_parts();
                        str_value.pop().unwrap()
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("SELECT {} FROM {}{};", &fields, self.schema(), where_str,)
            }
            SqlStatement::InsertOne(row) => {
                let (mut fields, mut bind_idx) = row.to_sql_parts();
                let fields_str = fields.join(", ");
                let bind_idx_str = bind_idx.unwrap().join(", ");
                format!(
                    "INSERT INTO {} ({}) VALUES ({}){} RETURNING *;",
                    self.schema(),
                    fields_str,
                    bind_idx_str,
                    where_str,
                )
            }

            SqlStatement::InsertMany(f) => {
                unimplemented!();
            }
            SqlStatement::Update(row) => {
                // TODO: this is with a Row, meaning it has an ID, the Where Statement will be
                // appended with id = x
                // TODO: downstream checks on uniqueness for Filters, Patch and Select.
                let where_str = sql_where(
                    &self.filters(),
                    self.statement().bind_len(),
                    Some("_x_".into()),
                );
                let (mut fields, mut bind_idx) = row.to_sql_parts();
                let f = fields.join(", ");
                let f_f = fields
                    .into_iter()
                    .map(|f| format!("{f} = _z_.{f}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let v = bind_idx
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_ SET {f_f} FROM (VALUES ({v})) AS _z_({f}) {where_str} RETURNING *;"
                )
            }
            SqlStatement::Patch(fields) => {
                // TODO: this is with a Row, meaning it has an ID, the Where Statement will be
                // appended with id = x
                // TODO: downstream checks on uniqueness for Filters, Patch and Select.
                let where_str = sql_where(
                    &self.filters(),
                    self.statement().bind_len(),
                    Some("_x_".into()),
                );
                // NOTE: the binding could not be calculated at that level. It has to be done
                // manually
                let (mut fields, _) =
                    concat_sql_parts(fields.iter().map(|f| f.to_sql_parts()).collect::<Vec<_>>());
                let bind_idx = 0..fields.len();
                let f = fields.join(", ");
                let f_f = fields
                    .iter()
                    .map(|f| format!("{f} = _z_.{f}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let v = bind_idx
                    .map(|i| format!("${:?}", i + 1))
                    .collect::<Vec<_>>()
                    .join(", ");
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_ SET {f_f} FROM (VALUES ({v})) AS _z_({f}) {where_str} RETURNING *;"
                )
            }
        }
    }
    // bind the arguments to the SQL query
    fn args(&self) -> sqlx::postgres::PgArguments {
        let mut args = sqlx::postgres::PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.statement().bind(&mut args);
        for w in self.filters().iter() {
            w.bind(&mut args);
        }
        args
    }
}

// Expose Execution methods
// Self has to impl ToSql so that it can access SQL Conversion Methods
pub trait Execute<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch>:
    ToSql<R, F, P>
{
    async fn execute(&self, ctx: &C) -> anyhow::Result<()> {
        todo!()
    }
    async fn fetch_optional(&self, ctx: &C) -> anyhow::Result<Option<A>> {
        todo!()
    }
    async fn fetch_one(&self, ctx: &C) -> anyhow::Result<A> {
        todo!()
    }
    async fn fetch_all(&self, ctx: &C) -> anyhow::Result<Vec<A>> {
        match &self.statement() {
            SqlStatement::Update(_) | SqlStatement::Patch(_) => {
                if *&self.filters().len() < 1 {
                    return Err(anyhow::anyhow!("Unable to Update/Patch without filters"));
                }
                if self.statement().bind_len() < 1 {
                    return Err(anyhow::anyhow!("Unable to Update/Patch with empty fields"));
                }
            }
            SqlStatement::Select(field_blocks) => {
                if field_blocks.len() != 1 {
                    panic!(
                        "Unable to use the fetch_all method while choosing which fields to return. Use the fetch_all_raw() method."
                    );
                }
            }
            _ => {}
        }
        let sql = self.to_sql();
        let req = sqlx::query_as_with::<'_, sqlx::Postgres, A, sqlx::postgres::PgArguments>(
            &sql,
            self.args(),
        );
        let res: anyhow::Result<Vec<A>> = req
            .fetch_all(ctx.db_pool())
            .await
            .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e));
        res
    }
}

// Display the SQL to the user.
impl<C: Context, A: QueryAs, R: ToRow, F: ToField, P: ToPatch> Display for Builder<C, A, R, F, P>
where
    Self: ToSql<R, F, P>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}
