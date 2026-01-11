use sqlx::Arguments;
use std::fmt::{self, Display};
use std::marker::PhantomData;

use crate::request_context::ContextAccessor;

pub trait FromRow: Display + ToSql + BindArgs {}

impl<F> FromRow for F where F: Display + ToSql + BindArgs {}

pub trait QueryAs: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}

impl<F> QueryAs for F where F: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}

pub struct MaeRepo<A: QueryAs, F: FromRow> {
    cmd_block: SqlCmd<F>,
    whr_block: Vec<WhereCondition<F>>,
    repo_name: String,
    from_row: PhantomData<fn() -> A>,
}

impl<A: QueryAs, F: FromRow> MaeRepo<A, F> {
    fn args(&self) -> sqlx::postgres::PgArguments {
        let mut args = sqlx::postgres::PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.cmd_block.bind(&mut args);
        for w in self.whr_block.iter() {
            w.bind(&mut args);
        }
        args
    }
    pub async fn fetch_all<CA: ContextAccessor + Send + Unpin>(
        &self,
        ctx: CA,
    ) -> anyhow::Result<Vec<A>> {
        let sql = self.cmd_block.sql(&self.repo_name, &self.whr_block);
        let req = sqlx::query_as_with::<'_, sqlx::Postgres, A, sqlx::postgres::PgArguments>(
            &sql,
            self.args(),
        );
        return req
            .fetch_all(ctx.db_pool())
            .await
            .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e));
    }
}

impl<A: QueryAs, F: FromRow> Display for MaeRepo<A, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sql = self.cmd_block.sql(&self.repo_name, &self.whr_block);
        write!(f, "{}", sql)
    }
}

pub enum SqlCmd<F: FromRow> {
    Select(Vec<F>),
    Insert(Vec<F>),
    Update(Vec<F>),
}

impl<F: FromRow> SqlCmd<F> {
    fn values(&self) -> String {
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
                .map(|f| f.sql_select())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    fn sql(&self, repo_name: &String, whr_block: &Vec<WhereCondition<F>>) -> String {
        match self {
            SqlCmd::Select(_) => {
                format!(
                    "SELECT {} FROM {}{};",
                    self.values(),
                    repo_name,
                    sql_where(whr_block),
                )
            }
            SqlCmd::Insert(_) => {
                format!(
                    "INSERT INTO {} {}{} RETURNING *;",
                    repo_name,
                    self.values(),
                    sql_where(whr_block),
                )
            }
            SqlCmd::Update(_) => {
                format!(
                    "UPDATE {} SET {} WHERE {} RETURNING *;",
                    repo_name,
                    self.values(),
                    sql_where(whr_block),
                )
            }
        }
    }
}

impl<F: FromRow> BindArgs for SqlCmd<F> {
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

pub enum WhereCondition<F: FromRow> {
    And(F, Where),
    Or(F, Where),
    Begin(F, Where),
}

impl<F: FromRow> Display for WhereCondition<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhereCondition::Begin(field, cond) => write!(f, "{} {}", field, cond),
            WhereCondition::And(field, cond) => write!(f, "{} AND {} {}", field, field, cond),
            WhereCondition::Or(field, cond) => write!(f, "{} OR {} {}", field, field, cond),
        }
    }
}

impl<F: FromRow> BindArgs for WhereCondition<F> {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args),
        }
    }
}

fn sql_where<F: FromRow>(w: &Vec<WhereCondition<F>>) -> String {
    let whr = w
        .iter()
        .map(|f| f.to_string())
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

pub trait Builder<C: Clone, F: FromRow>: QueryAs + KeyAuths<F> {
    fn build(cmd_block: SqlCmd<F>) -> MaeRepo<Self, F> {
        MaeRepo::<Self, F> {
            cmd_block,
            whr_block: Self::keys(),
            repo_name: Self::repo_name(),
            from_row: PhantomData,
        }
    }
    fn repo_name() -> String;
}

pub trait KeyAuths<F: FromRow>: QueryAs {
    fn keys() -> Vec<WhereCondition<F>>;
}

pub trait Interface<C: Clone, F: FromRow>: QueryAs + Builder<C, F> {
    fn insert_many(recs: Vec<F>) -> MaeRepo<Self, F> {
        Self::build(SqlCmd::Insert(recs))
    }

    fn select_many(recs: Vec<F>) -> MaeRepo<Self, F> {
        Self::build(SqlCmd::Select(recs))
    }
}
