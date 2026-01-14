use std::fmt::Display;
use std::marker::PhantomData;

use super::map_util::{BindArgs, FilterOp, SqlStatement, ToSql, sql_where};
use super::type_def::{Context, QueryAs, ToField, ToRow};
use crate::request_context::ContextAccessor;

// /////
// INTERFACE TO THE SCHEMA def
//  ////

pub trait Build<C: Context, T: ToRow, F: ToField>: QueryAs + KeyAuths<F> {
    // Get a builder so we can build some SQL
    fn build(statement: SqlStatement<T, F>) -> Builder<C, Self, T, F> {
        Builder::<C, Self, T, F> {
            statement,
            filters: Self::keys(),
            table_ident: Self::table_ident(),
            ctx: PhantomData,
            query_as: PhantomData,
        }
    }
    // Extend a method so we can access the SQL Schema
    fn table_ident() -> String;
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
pub trait Interface<C: Context, T: ToRow, F: ToField>: Build<C, T, F> {
    fn insert_one(rec: T) -> Builder<C, Self, T, F> {
        Self::build(SqlStatement::<T, F>::InsertOne(rec))
    }

    fn insert_many(recs: Vec<T>) -> Builder<C, Self, T, F> {
        Self::build(SqlStatement::<T, F>::InsertMany(recs))
    }

    fn select(recs: Vec<F>) -> Builder<C, Self, T, F> {
        Self::build(SqlStatement::<T, F>::Select(recs))
    }
    fn update_many(rec: T) -> Builder<C, Self, T, F> {
        Self::build(SqlStatement::Update(rec))
    }
    fn patch() {
        //TODO: patch requires a new type to be add, tyoed Enum
        todo!()
    }
}

// Anything that implements Build `B` implements the Interface
impl<C: Context, T: ToRow, F: ToField, B: Build<C, T, F>> Interface<C, T, F> for B {}

// ////
// BUILDER MECHANICS
// ////

// Our builder will be composed of multiple parts
pub struct Builder<C: Context, A: QueryAs, T: ToRow, F: ToField> {
    // SELECT, UPDATE, INSERT ...
    statement: SqlStatement<T, F>,
    // WHERE ...
    filters: Vec<FilterOp<F>>,
    // Schema
    table_ident: String,
    // Context, required for sql executions
    ctx: PhantomData<fn() -> C>,
    // QueryAs, required for sql executions
    query_as: PhantomData<fn() -> A>, // TODO: Add offset/limit's
}

// expose build methods with the *Builder Pattern*
impl<C: Context, A: QueryAs, T: ToRow, F: ToField> Builder<C, A, T, F> {
    // add filters to the query
    pub fn filter(mut self, mut values: Vec<FilterOp<F>>) -> Self {
        self.filters.append(&mut values);
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
impl<C: Context, A: QueryAs, T: ToRow, F: ToField> RenameMetoToSqlRemoveOther<T, F>
    for Builder<C, A, T, F>
{
    fn statement(&self) -> &SqlStatement<T, F> {
        &self.statement
    }
    fn table_ident(&self) -> &String {
        &self.table_ident
    }
    fn filters(&self) -> &Vec<FilterOp<F>> {
        &self.filters
    }
}

// The Builder can execute SQL Queries
// Self is required to be RenameMetoToSqlRemoveOther so that it can access it's conversion methods
impl<C: Context, A: QueryAs, T: ToRow, F: ToField> Execute<C, A, T, F> for Builder<C, A, T, F> where
    Self: RenameMetoToSqlRemoveOther<T, F>
{
}

// Turn the type into an SQL Statement, from parts
pub trait RenameMetoToSqlRemoveOther<T: ToRow, F: ToField> {
    // get the INSERT, SELECT, UPDATE method
    fn statement(&self) -> &SqlStatement<T, F>;
    // get the WHERE method
    fn filters(&self) -> &Vec<FilterOp<F>>;
    // get the schema name
    fn table_ident(&self) -> &String;
    // TODO: this function is going to have to do some more leg work; the ToSql trait's method
    // fields() should return the field names + the binding labels, handling the building of the
    // sql statement here... hense the 'to_sql' method. change the fields() method name to
    // fields_parts or maybe split the function into two. then build the sql statement here,
    // locally. This will prevent some pain points in passing useless strings everywhere.
    // Also, the Returning statements should be optional along with appending offset and limits
    fn to_sql(&self) -> String {
        let where_str = sql_where(&self.filters(), self.statement().bind_len());
        match &self.statement() {
            SqlStatement::Select(_) => {
                // NOTE: the sql_where fn has a fixed idx at 0; the prefix of the statement always
                // takes no arguements.

                // NOTE: if the values return an empty string, select everything.
                let mut fields = self.statement().fields();
                if fields.is_empty() {
                    fields = "*".into();
                }
                format!(
                    "SELECT {} FROM {}{};",
                    fields,
                    self.table_ident(),
                    sql_where(&self.filters(), 0),
                )
            }
            SqlStatement::InsertOne(f) => {
                format!(
                    "INSERT INTO {} {}{} RETURNING *;",
                    self.table_ident(),
                    self.statement().fields(),
                    sql_where(&self.filters(), self.statement().bind_len()),
                )
            }
            SqlStatement::InsertMany(f) => {
                unimplemented!("see comment for this method.");
                format!(
                    "INSERT INTO {} {}{} RETURNING *;",
                    self.table_ident(),
                    self.statement().fields(),
                    where_str
                )
            }
            SqlStatement::Update(f) => {
                format!(
                    "UPDATE {} SET {}{} RETURNING *;",
                    self.table_ident(),
                    self.statement().fields(),
                    sql_where(&self.filters(), self.statement().bind_len()),
                )
            }
            SqlStatement::Patch(f) => {
                unimplemented!("see comment for this method.");
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
// Self has to impl RenameMetoToSqlRemoveOther so that it can access SQL Conversion Methods
pub trait Execute<C: Context, A: QueryAs, T: ToRow, F: ToField>:
    RenameMetoToSqlRemoveOther<T, F>
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
            SqlStatement::Update(_) => {
                if *&self.filters().len() < 1 {
                    return Err(anyhow::anyhow!("Unable to Update without filters"));
                }
                if self.statement().bind_len() < 1 {
                    return Err(anyhow::anyhow!("Unable to Update with empty fields"));
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
impl<C: Context, A: QueryAs, T: ToRow, F: ToField> Display for Builder<C, A, T, F>
where
    Self: RenameMetoToSqlRemoveOther<T, F>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}
