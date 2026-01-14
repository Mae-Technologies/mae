use sqlx::Arguments;
use std::fmt::{self, Display};
use std::marker::PhantomData;

use crate::request_context::ContextAccessor;

pub trait ToRow: Display + ToSql + BindArgs {}

impl<T> ToRow for T where T: Display + ToSql + BindArgs {}

pub trait Filter: Display {}
impl<F> Filter for F where F: Display {}

pub trait QueryAs: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}

impl<F> QueryAs for F where F: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}

pub struct Builder<A: QueryAs, T: ToRow, F: Filter> {
    statement: SqlStatement<T, F>,
    filters: Vec<WhereCondition<F>>,
    table_ident: String,
    from_row: PhantomData<fn() -> A>,
}

impl<A: QueryAs, T: ToRow, F: Filter> Builder<A, T, F> {
    fn to_sql(&self) -> String {
        match &self.statement {
            SqlStatement::Select(_) => {
                // NOTE: the sql_where fn has a fixed idx at 0; the prefix of the statement always
                // takes no arguements.

                // NOTE: if the values return an empty string, select everything.
                let mut fields = self.statement.fields();
                if fields.is_empty() {
                    fields = "*".into();
                }
                format!(
                    "SELECT {} FROM {}{};",
                    fields,
                    self.table_ident,
                    sql_where(&self.filters, 0),
                )
            }
            SqlStatement::Insert(f) => {
                format!(
                    "INSERT INTO {} {}{} RETURNING *;",
                    self.table_ident,
                    self.statement.fields(),
                    sql_where(&self.filters, f.len()),
                )
            }
            SqlStatement::Update(f) => {
                format!(
                    "UPDATE {} SET {}{} RETURNING *;",
                    self.table_ident,
                    self.statement.fields(),
                    sql_where(&self.filters, f.len()),
                )
            }
        }
    }
    fn args(&self) -> sqlx::postgres::PgArguments {
        let mut args = sqlx::postgres::PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.statement.bind(&mut args);
        for w in self.filters.iter() {
            w.bind(&mut args);
        }
        args
    }
    pub async fn fetch_all<CA: ContextAccessor + Send + Unpin>(
        &self,
        ctx: CA,
    ) -> anyhow::Result<Vec<A>> {
        let sql = self.to_sql();
        let req = sqlx::query_as_with::<'_, sqlx::Postgres, A, sqlx::postgres::PgArguments>(
            &sql,
            self.args(),
        );
        return req
            .fetch_all(ctx.db_pool())
            .await
            .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e));
    }

    pub fn filter(mut self, mut values: Vec<WhereCondition<F>>) -> Self {
        self.filters.append(&mut values);
        self
    }
}

impl<A: QueryAs, T: ToRow, F: Filter> Display for Builder<A, T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

pub enum SqlStatement<T: ToRow, F: Filter> {
    Select(Vec<F>),
    Insert(Vec<T>),
    Update(Vec<T>),
}

impl<T: ToRow, F: Filter> SqlStatement<T, F> {
    fn fields(&self) -> String {
        // NOTE: we don't need to convert and map the bindings for a select statement. to_string
        // will do.
        match self {
            Self::Insert(v) => v
                .iter()
                .map(|f| f.sql_insert())
                .collect::<Vec<_>>()
                .join(" "),
            Self::Update(v) => v
                .iter()
                .map(|f| f.sql_update())
                .collect::<Vec<_>>()
                .join(" "),
            Self::Select(v) => v
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

impl<T: ToRow, F: Filter> BindArgs for SqlStatement<T, F> {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Select(v) => {
                for ele in v {
                    let _ = args.add(ele.to_string());
                }
            }
            Self::Insert(v) => {
                for ele in v {
                    ele.bind(args);
                }
            }
            Self::Update(v) => {
                for ele in v {
                    ele.bind(args);
                }
            }
        }
    }
}

pub enum WhereCondition<F: Filter> {
    And(F, Where),
    Or(F, Where),
    Begin(F, Where),
}

impl<F: Filter> Display for WhereCondition<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhereCondition::Begin(field, cond) => write!(f, "{} {}", field, cond),
            WhereCondition::And(field, cond) => write!(f, "AND {} {}", field, cond),
            WhereCondition::Or(field, cond) => write!(f, "OR {} {}", field, cond),
        }
    }
}

impl<F: Filter> BindArgs for WhereCondition<F> {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args),
        }
    }
}

fn sql_where<F: Filter>(w: &Vec<WhereCondition<F>>, idx: usize) -> String {
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

pub trait BindArgs {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments);
}

pub trait ToSql {
    fn sql_insert(&self) -> String;
    fn sql_update(&self) -> String;
    fn sql_select(&self) -> String;
}

pub trait Build<C: Clone, T: ToRow, F: Filter>: QueryAs + KeyAuths<F> {
    fn build(statement: SqlStatement<T, F>) -> Builder<Self, T, F> {
        Builder::<Self, T, F> {
            statement,
            filters: Self::keys(),
            table_ident: Self::table_ident(),
            from_row: PhantomData,
        }
    }
    fn table_ident() -> String;
}

pub trait KeyAuths<F: Filter>: QueryAs {
    fn keys() -> Vec<WhereCondition<F>>;
}

pub trait Interface<C: Clone, T: ToRow, F: Filter>: QueryAs + Build<C, T, F> {
    fn insert_many(recs: Vec<T>) -> Builder<Self, T, F> {
        Self::build(SqlStatement::<T, F>::Insert(recs))
    }

    fn select(recs: Vec<F>) -> Builder<Self, T, F> {
        Self::build(SqlStatement::<T, F>::Select(recs))
    }
}
