use sqlx::Arguments;
use std::fmt::{self, Display};
use std::marker::PhantomData;

use crate::request_context::ContextAccessor;

// /////
// TYPES
//  ////

//  SOMETHING THAT WILL HAVE OUR CONTEXT -> C
pub trait Context: ContextAccessor + Unpin + Send {}
impl<T> Context for T where T: ContextAccessor + Unpin + Send {}

// SOMETHING THAT WILL CONVERT TO A ROW -> T
pub trait ToRow: ToSql + BindArgs {}
impl<T> ToRow for T where T: ToSql + BindArgs {}

// SOMETHING THAT WILL CONVERT TO A FIELD -> F
pub trait Field: Display {}
impl<F> Field for F where F: Display {}

// SOMETHING THAT AN SQL ROW CAN BE CONVERTED INTO -> A
pub trait QueryAs: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}
impl<A> QueryAs for A where A: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}

// /////
// INTERFACE TO THE STRUCT def
//  ////

pub trait Build<C: Context, T: ToRow, F: Field>: QueryAs + KeyAuths<F> {
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
pub trait KeyAuths<F: Field> {
    fn keys() -> Vec<FilterOp<F>>;
}

// Expose methods to the user defined struct
pub trait Interface<C: Context, T: ToRow, F: Field>: Build<C, T, F> {
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
impl<C: Context, T: ToRow, F: Field, B: Build<C, T, F>> Interface<C, T, F> for B {}

// ////
// BUILDER MECHANICS
// ////

// Our builder will be composed of multiple parts
pub struct Builder<C: Context, A: QueryAs, T: ToRow, F: Field> {
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
impl<C: Context, A: QueryAs, T: ToRow, F: Field> Builder<C, A, T, F> {
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
impl<C: Context, A: QueryAs, T: ToRow, F: Field> RenameMetoToSqlRemoveOther<T, F>
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
impl<C: Context, A: QueryAs, T: ToRow, F: Field> Execute<C, A, T, F> for Builder<C, A, T, F> where
    Self: RenameMetoToSqlRemoveOther<T, F>
{
}

// Turn the type into an SQL Statement, from parts
pub trait RenameMetoToSqlRemoveOther<T: ToRow, F: Field> {
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
pub trait Execute<C: Context, A: QueryAs, T: ToRow, F: Field>:
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
impl<C: Context, A: QueryAs, T: ToRow, F: Field> Display for Builder<C, A, T, F>
where
    Self: RenameMetoToSqlRemoveOther<T, F>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

// /////
// INTERNAL: CONVERT THE BUILDER PARTS TO SQL
// /////

// There are two interfaces to all the SQL parts:

//  - BindArgs
//      If there are arguments, they need to be safely inserted into the SQL Query with PgArguments
pub trait BindArgs {
    // TODO: is any of these are to panic, this method should return a Result
    fn bind(&self, args: &mut sqlx::postgres::PgArguments);
    fn bind_len(&self) -> usize;
}

//  - ToSql
//      If there are column representations inside the types, they need to be extracted.
//      This is done with the Dispay inpl
pub trait ToSql {
    fn sql_insert(&self) -> String;
    fn sql_update(&self) -> String;
    fn sql_patch(&self) -> String;
    fn sql_select(&self) -> String;
}

// SQL Statements
enum SqlStatement<T: ToRow, F: Field> {
    Select(Vec<F>),
    InsertOne(T),
    InsertMany(Vec<T>),
    Update(T),
    // TODO: Patch<Vec<T>> should be a new type (typed variants)
    Patch(Vec<T>),
}

// TODO: This should probably follow the ToSql impl
impl<T: ToRow, F: Field> SqlStatement<T, F> {
    // get the statement parts
    fn fields(&self) -> String {
        // TODO: this isn't very intuitive, (1) fields is not very descriptive, (2) returning a
        // string is not helpful. When building the sql string, we will need more details so the
        // method is more helpful (IE, current update impl only works for one row, and same with
        // insert) rename this function to sql() -> String, String
        // NOTE: we don't need to convert and map the bindings for a select statement. to_string
        // will do.
        match self {
            Self::InsertOne(v) => v.sql_insert(),
            Self::InsertMany(v) => v
                .iter()
                .map(|f| f.sql_insert())
                .collect::<Vec<_>>()
                .join(", "),
            Self::Update(v) => v.sql_update(),
            Self::Patch(v) => v
                .iter()
                .map(|f| f.sql_patch())
                .collect::<Vec<_>>()
                .join(", "),
            Self::Select(v) => v
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

impl<T: ToRow, F: Field> BindArgs for SqlStatement<T, F> {
    // Bind the Statement values to the query
    // (Ie - Struct{value: 1} or Enum::value(1)) -> iter.v / v -> PgArguments.add(v)
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Select(v) => {
                for ele in v {
                    let _ = args.add(ele.to_string());
                }
            }
            Self::InsertOne(v) => v.bind(args),
            Self::InsertMany(v) => v.iter().for_each(|f| f.bind(args)),
            Self::Update(v) => v.bind(args),
            Self::Patch(v) => v.iter().for_each(|f| f.bind(args)),
        }
    }

    // Get the count of arg's that are to be bound
    fn bind_len(&self) -> usize {
        match self {
            Self::Update(v) | Self::InsertOne(v) => v.bind_len(),
            // NOTE: There are no bindings for select statements
            Self::Select(_) => 0,
            Self::InsertMany(v) => v.iter().map(|v| v.bind_len()).sum(),
            Self::Patch(v) => v.len(),
        }
    }
}

// Filter / Where block of the Query
pub enum Where {
    Equals(i32),
    NotEquals(i32),
    In(Vec<i32>),
    NotIn(Vec<i32>),
    Like(String),
    NotLike(String),
    Ilike(String),
    NotIlike(String),
    StringIs(String),
    StringIsNot(String),
    Gt(i32),
    Gte(i32),
    Lt(i32),
    Lte(i32),
    IsNull,
}

impl BindArgs for Where {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        let _ = match self {
            Self::Equals(v) => args.add(v),
            Self::NotEquals(v) => args.add(v),
            Self::In(v) => args.add(v),
            Self::NotIn(v) => args.add(v),
            Self::Like(v) => args.add(v),
            Self::NotLike(v) => args.add(v),
            Self::Ilike(v) => args.add(v),
            Self::NotIlike(v) => args.add(v),
            Self::StringIs(v) => args.add(v),
            Self::StringIsNot(v) => args.add(v),
            Self::Gt(v) => args.add(v),
            Self::Gte(v) => args.add(v),
            Self::Lt(v) => args.add(v),
            Self::Lte(v) => args.add(v),
            Self::IsNull => Ok(()),
        };
    }
    fn bind_len(&self) -> usize {
        1
    }
}

impl Display for Where {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Where::Equals(_) => write!(f, "="),
            Where::NotEquals(_) => write!(f, "!="),
            Where::In(_) => write!(f, "IN"),
            Where::NotIn(_) => write!(f, "NOT IN"),
            Where::Like(_) => write!(f, "LIKE"),
            Where::NotLike(_) => write!(f, "NOT LIKE"),
            Where::Ilike(_) => write!(f, "ILIKE"),
            Where::NotIlike(_) => write!(f, "NOT ILIKE"),
            Where::StringIs(_) => write!(f, "="),
            Where::StringIsNot(_) => write!(f, "!="),
            Where::Gt(_) => write!(f, ">"),
            Where::Gte(_) => write!(f, ">="),
            Where::Lt(_) => write!(f, "<"),
            Where::Lte(_) => write!(f, "<="),
            Where::IsNull => write!(f, "IS NULL"),
        }
    }
}

// Filter / Where Operators
pub enum FilterOp<F: Field> {
    And(F, Where),
    Or(F, Where),
    Begin(F, Where),
}

impl<F: Field> Display for FilterOp<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterOp::Begin(field, cond) => write!(f, "{} {}", field, cond),
            FilterOp::And(field, cond) => write!(f, "AND {} {}", field, cond),
            FilterOp::Or(field, cond) => write!(f, "OR {} {}", field, cond),
        }
    }
}

impl<F: Field> BindArgs for FilterOp<F> {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args),
        }
    }
    fn bind_len(&self) -> usize {
        match self {
            Self::Begin(_, w) => w.bind_len(),
            Self::And(_, w) => w.bind_len(),
            Self::Or(_, w) => w.bind_len(),
        }
    }
}

// Static method to extract the Where block of the Sql Query. They will always be the same / have
// the same structure
fn sql_where<F: Field>(w: &Vec<FilterOp<F>>, idx: usize) -> String {
    let whr = w
        .iter()
        .zip(1..)
        .map(|(f, i)| format!("{} ${}", f.to_string(), i + idx))
        .collect::<Vec<_>>()
        .join(" ");
    if !whr.is_empty() {
        return format!(" WHERE {}", whr);
    }
    whr
}
