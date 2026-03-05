use anyhow::{Ok, Result, anyhow};
use num::Zero;
use std::fmt::Debug;
use std::marker::PhantomData;

use sqlx::{Arguments, Executor, Postgres};

use crate::repo::filter::Filter;
use crate::request_context::ContextAccessor;

use super::map_util::{BindArgs, FilterOp, SqlStatement, concat_sql_parts, sql_where};
use super::type_def::{Context, QueryAs, ToField, ToInsertRow, ToPatch, ToUpdateRow};

// /////
// INTERFACE TO THE SCHEMA def
//  ////

pub trait Build<C: Context, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>:
    QueryAs + KeyAuths<F,>
{
    // Get a builder so we can build some SQL
    fn build(ctx: &C, statement: SqlStatement<I, U, F, P,>,) -> Builder<'_, C, Self, I, U, F, P,> {
        Builder::<C, Self, I, U, F, P,> {
            statement,
            filters: Self::keys(),
            schema: Self::schema(),
            ctx,
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
// TODO: the recs / rec needs to be borrowed -- this brings in lifetimes
pub trait Interface<C: Context, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>:
    Build<C, I, U, F, P,>
{
    fn insert_one(ctx: &C, rec: I,) -> Builder<'_, C, Self, I, U, F, P,> {
        Self::build(ctx, SqlStatement::<I, U, F, P,>::InsertOne(rec,),)
    }

    fn insert_many(ctx: &C, recs: Vec<I,>,) -> Builder<'_, C, Self, I, U, F, P,> {
        Self::build(ctx, SqlStatement::<I, U, F, P,>::InsertMany(recs,),)
    }

    fn select(ctx: &C, rec: Vec<F,>,) -> Builder<'_, C, Self, I, U, F, P,> {
        Self::build(ctx, SqlStatement::<I, U, F, P,>::Select(rec,),)
    }
    fn update_many(ctx: &C, rec: U,) -> Builder<'_, C, Self, I, U, F, P,> {
        Self::build(ctx, SqlStatement::Update(rec,),)
    }
    fn patch(ctx: &C, recs: Vec<P,>,) -> Builder<'_, C, Self, I, U, F, P,> {
        Self::build(ctx, SqlStatement::Patch(recs,),)
    }
}

// Anything that implements Build `B` implements the Interface
impl<C: Context, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch, B: Build<C, I, U, F, P,>,>
    Interface<C, I, U, F, P,> for B
{
}

// ////
// BUILDER MECHANICS
// ////

// Our builder will be composed of multiple parts
pub struct Builder<
    'a,
    C: Context,
    A: QueryAs,
    I: ToInsertRow,
    U: ToUpdateRow,
    F: ToField,
    P: ToPatch,
> {
    // SELECT, UPDATE, INSERT ...
    statement: SqlStatement<I, U, F, P,>,
    // WHERE ...
    filters: Vec<FilterOp<F,>,>,
    // Schema
    schema: String,
    // Context, required for sql executions
    ctx: &'a C,
    // QueryAs, required for sql executions
    query_as: PhantomData<fn() -> A,>,
    //  what are we returning? (Insert / Update)
    returning: Option<Vec<F,>,>,
    // TODO: Add offset/limit's
}

// expose build methods with the *Builder Pattern*
impl<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>
    Builder<'_, C, A, I, U, F, P,>
{
    // add filters to the query
    pub fn filter(mut self, mut values: Vec<FilterOp<F,>,>,) -> Self {
        self.filters.append(&mut values,);
        self
    }
    // TODO: maybe returning isn't a feature that we want; we always return the struct, cannot return
    // a custom set of rows.
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

// the builder needs to access context
impl<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>
    ContextAccessor for Builder<'_, C, A, I, U, F, P,>
{
    fn db_pool(&self,) -> &sqlx::PgPool {
        self.ctx.db_pool()
    }
    fn session(&self,) -> &crate::session::Session {
        self.ctx.session()
    }
    fn session_user(&self,) -> &i32 {
        self.ctx.session_user()
    }
}

// The builder has to convert-to and execute SQL Queries:
//

// The Builder can convert to SQL Queries
impl<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>
    ToSql<I, U, F, P,> for Builder<'_, C, A, I, U, F, P,>
{
    fn statement(&self,) -> &SqlStatement<I, U, F, P,> {
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
impl<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>
    Execute<C, A, I, U, F, P,> for Builder<'_, C, A, I, U, F, P,>
where
    Self: ToSql<I, U, F, P,>,
{
}

// Turn the type into an SQL Statement, from parts
pub trait ToSql<I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,> {
    // get the INSERT, SELECT, UPDATE method
    fn statement(&self,) -> &SqlStatement<I, U, F, P,>;
    // get the WHERE method
    fn filters(&self,) -> &Vec<FilterOp<F,>,>;
    // get the schema name
    fn schema(&self,) -> &String;
    fn to_sql(&self,) -> Result<String,> {
        // TODO: The Returning cmd is hard coded for now. There has to be a new impl to get all fields.
        // TODO: move the ctx. into a defualt KeyAuths impl, that way it display's in our Display impl automatically
        Ok(match &self.statement() {
            SqlStatement::Select(field_blocks,) => {
                let where_str = sql_where(self.filters(), self.statement().bind_len(), None,);
                let fields = field_blocks
                    .iter()
                    .map(|field| -> Result<String,> {
                        let (mut str_value, _,) = field.to_sql_parts();
                        str_value.pop().ok_or_else(|| anyhow!("cannot find binding index"),)
                    },)
                    .collect::<Result<Vec<_,>,>>()?
                    .join(",\n\t",);
                // WARN: no need to add context, there is no DML
                format!("SELECT\n\t{}\nFROM {}{};", &fields, self.schema(), where_str,)
            }
            SqlStatement::InsertOne(row,) => {
                // bind_len + 1 for the context
                let (mut fields, bind_idx_option,) = row.to_sql_parts();
                let mut bind_idx =
                    bind_idx_option.ok_or_else(|| anyhow!("cannot find binding index"),)?;

                // NOTE: adding context
                // WARN: this expects another part to actually bind the $x variable to the
                // statement
                fields.push("created_by".into(),);
                let last_idx = bind_idx.len();
                bind_idx.push(format!("${}", last_idx + 1),);

                let fields_str = fields.join(",\n\t ",);
                let bind_idx_str = bind_idx.join(", ",);
                format!(
                    "INSERT INTO {}\n\t(\n\t {}\n\t)\n\tVALUES ({})\nRETURNING *;",
                    self.schema(),
                    fields_str,
                    bind_idx_str,
                )
            }

            SqlStatement::InsertMany(_,) => {
                unimplemented!();
            }
            SqlStatement::Update(row,) => {
                // Bind len + 1 for context
                let where_str = sql_where(
                    self.filters(),
                    self.statement().bind_len() + 1,
                    Some("_x_".into(),),
                );
                let (mut fields, bind_idx_option,) = row.to_sql_parts();
                let mut bind_idx =
                    bind_idx_option.ok_or_else(|| anyhow!("cannot find binding index"),)?;

                // NOTE: adding context
                // WARN: this expects another part to actually bind the $x variable to the
                // statement
                fields.push("updated_by".into(),);
                let last_idx = bind_idx.len();
                bind_idx.push(format!("${}", last_idx + 1),);

                let f = fields.join(",\n\t\t ",);
                let f_f = fields
                    .into_iter()
                    .map(|f| format!("\n\t\t{f} = _z_.{f}"),)
                    .collect::<Vec<_,>>()
                    .join(", ",);
                let v = bind_idx.join(", ",);
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_\n\tSET {f_f}\n\tFROM\n\t\t(VALUES ({v}))\n\tAS _z_ (\n\t\t {f}\n\t\t){where_str}\nRETURNING *;"
                )
            }
            SqlStatement::Patch(fields,) => {
                // Bind len + 1 for context
                let where_str = sql_where(
                    self.filters(),
                    self.statement().bind_len() + 1,
                    Some("_x_".into(),),
                );
                // NOTE: the binding could not be calculated at the patch level. It has to be done
                // manually
                let (mut fields, _,) = concat_sql_parts(
                    fields.iter().map(|f| f.to_sql_parts(),).collect::<Vec<_,>>(),
                );
                let mut bind_idx =
                    (0..fields.len()).map(|i| format!("${:?}", i + 1),).collect::<Vec<_,>>();

                // NOTE: adding context
                // WARN: this expects another part to actually bind the $x variable to the
                // statement
                fields.push("updated_by".into(),);
                let last_idx = bind_idx.len();
                bind_idx.push(format!("${}", last_idx + 1),);

                let f = fields.join(",\n\t\t ",);
                let f_f = fields
                    .into_iter()
                    .map(|f| format!("\n\t\t{f} = _z_.{f}"),)
                    .collect::<Vec<_,>>()
                    .join(", ",);
                let v = bind_idx.join(", ",);
                let schema = self.schema();
                format!(
                    "UPDATE {schema} _x_\n\tSET {f_f}\n\tFROM\n\t\t(VALUES ({v}))\n\tAS _z_ (\n\t\t {f}\n\t\t){where_str}\nRETURNING *;"
                )
            }
        },)
    }
    // bind the arguments to the SQL query
    fn args(&self, session_user: &i32,) -> sqlx::postgres::PgArguments {
        let mut args = sqlx::postgres::PgArguments::default();

        match self.statement() {
            SqlStatement::Select(_,) => {}
            _ => {
                // NOTE: we need to bind the args before the where statement.
                self.statement().bind(&mut args,);
                // NOTE: then we need to bind context
                let _ = args.add(session_user,);
            }
        };

        // NOTE: finally, we can bind the filters
        for w in self.filters().iter() {
            w.bind(&mut args,);
        }
        // TODO: bind the returning, offset, limits
        args
    }
}

// Expose Execution methods
// Self has to impl ToSql so that it can access SQL Conversion Methods
pub trait Execute<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,>:
    ToSql<I, U, F, P,> + ContextAccessor
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
        Self: Sync + Send,
    {
        async move {
            self.authenticate_request()?;
            let sql = self.to_sql()?;

            let req = sqlx::query_as_with::<'_, sqlx::Postgres, A, sqlx::postgres::PgArguments,>(
                &sql,
                self.args(self.session_user(),),
            );
            let res: anyhow::Result<Vec<A,>,> = req
                .fetch_all(exec,)
                .await
                .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e),);
            res
        }
    }
    // TODO: downstream checks on uniqueness for CRUD Operations, meaning, filter and patch lists must be uniqueness
    fn authenticate_request(&self,) -> Result<(),> {
        match self.statement() {
            SqlStatement::Update(_,) | SqlStatement::Patch(_,) => {
                if self.filters().is_empty() {
                    return Err(anyhow!("Unable to Update/Patch without filters"),);
                }
                if self.statement().bind_len() < 1 {
                    return Err(anyhow!("Unable to Update/Patch with all fields empty"),);
                }
                Ok((),)
            }
            SqlStatement::Select(field_blocks,) => {
                if field_blocks.len() != 1 {
                    return Err(anyhow!(
                        "Unable to use the fetch_all method while choosing which fields to return. Use the fetch_all_raw() method."
                    ),);
                }
                Ok((),)
            }
            _ => Ok((),),
        }
    }
}

// Display the SQL to the user.
impl<C: Context, A: QueryAs, I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch,> Debug
    for Builder<'_, C, A, I, U, F, P,>
where
    Self: ToSql<I, U, F, P,>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_,>,) -> std::fmt::Result {
        // statement title
        write!(f, "\n{}\n\tSQL\n{}\n\n", "*".repeat(18), "*".repeat(18),)?;

        // sql
        write!(f, "{}", self.to_sql().map_err(|_| std::fmt::Error)?)?;
        let mut bind_len = self.statement.bind_len();
        let mut _has_bindings = false;
        if !bind_len.is_zero() {
            _has_bindings = true;
            write!(f, "\n\n{}\n{}BINDINGS\n{}\n", "*".repeat(18), " ".repeat(5), "*".repeat(18),)?;
        }
        // binding values $1 ... inside statement
        if !bind_len.is_zero() {
            self.statement.fmt(f,)?;
        }

        // TODO: remove this block then context is added in properly
        match self.statement {
            SqlStatement::Select(_,) => writeln!(f),
            _ => {
                bind_len += 1;
                return write!(f, "\n\t${} = [session_user]\n", bind_len);
            }
        }?;

        // TODO: add this block then context is added in properly
        // write!(f, "\n")?;

        let mut filter_has_bindings = false;
        let mut filter_bindings_string = String::from("",);

        // binding values $1... inside where
        for (i, filter,) in self.filters.iter().enumerate() {
            match filter {
                FilterOp::And(_c, v,) | FilterOp::Or(_c, v,) | FilterOp::Begin(_c, v,) => match v {
                    Filter::IsNull => {}
                    _ => {
                        _has_bindings = true;
                        filter_has_bindings = true;
                        filter_bindings_string.push_str(&format!(
                            "\n\t${} = {:?}",
                            i + bind_len + 1,
                            &filter
                        ),);
                    }
                },
            }
        }
        if filter_has_bindings {
            if bind_len.is_zero() {
                write!(
                    f,
                    "\n\n{}\n{}BINDINGS\n{}\n",
                    "*".repeat(18),
                    " ".repeat(5),
                    "*".repeat(18),
                )?;
            }
            write!(f, "{}", filter_bindings_string)?;
            writeln!(f)?;
        }

        if bind_len.is_zero() {
            writeln!(f)?;
        }

        // closure
        write!(f, "\n{}", "*".repeat(18))?;
        write!(f, "\n{}\n", "*".repeat(18))?;
        std::fmt::Result::Ok((),)
    }
}
