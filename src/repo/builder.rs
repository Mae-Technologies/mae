use sqlx::Postgres;
use sqlx::postgres::PgArguments;
use sqlx::{Arguments, query::QueryAs};
use std::fmt::{self, Display};

use crate::request_context::ContextAccessor;

pub trait Field:
    Display + ToSql + BindArgs + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin
{
}

impl<F> Field for F where
    F: Display + ToSql + BindArgs + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin
{
}

pub struct MaeRepo<F: Field> {
    cmd_block: SqlCmd<F>,
    whr_block: Vec<WhereCondition<F>>,
    repo_name: String,
}

impl<F: Field> MaeRepo<F> {
    fn args(&self) -> PgArguments {
        let mut args = PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.cmd_block.bind(&mut args);
        for w in self.whr_block.iter() {
            w.bind(&mut args);
        }
        args
    }
    async fn fetch_all<CA: ContextAccessor + Send + Unpin>(
        &self,
        ctx: CA,
    ) -> anyhow::Result<Vec<F>> {
        let sql = self.cmd_block.sql(&self.repo_name, &self.whr_block);
        let req = sqlx::query_as_with::<'_, Postgres, F, PgArguments>(&sql, self.args());
        return req
            .fetch_all(ctx.db_pool())
            .await
            .map_err(|e| anyhow::anyhow!("Unable to fetch all: {}", e));
    }
}

impl<F: Field> Display for MaeRepo<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

pub enum SqlCmd<F: Field> {
    Select(Vec<F>),
    Insert(Vec<F>),
    Update(Vec<F>),
}

impl<F: Field> SqlCmd<F> {
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
                    "SELECT {} FROM {} WHERE {};",
                    self.values(),
                    repo_name,
                    sql_where(whr_block),
                )
            }
            SqlCmd::Insert(_) => {
                format!(
                    "INSERT INTO {} VALUES {} WHERE {} RETURNING *;",
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

impl<F: Field> BindArgs for SqlCmd<F> {
    fn bind(&self, args: &mut PgArguments) {
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

pub enum WhereCondition<F: Field> {
    And(F, Where),
    Or(F, Where),
    Begin(F, Where),
}

impl<F: Field> Display for WhereCondition<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhereCondition::Begin(field, cond) => write!(f, "{} {}", field, cond),
            WhereCondition::And(field, cond) => write!(f, "{} AND {} {}", field, field, cond),
            WhereCondition::Or(field, cond) => write!(f, "{} OR {} {}", field, field, cond),
        }
    }
}

impl<F: Field> BindArgs for WhereCondition<F> {
    fn bind(&self, args: &mut PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args),
        }
    }
}

fn sql_where<F: Field>(w: &Vec<WhereCondition<F>>) -> String {
    w.iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(" ")
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
    fn bind(&self, args: &mut PgArguments) {
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
    fn bind(&self, args: &mut PgArguments);
}

pub trait ToSql {
    fn sql_insert(&self) -> String;
    fn sql_update(&self) -> String;
    fn sql_select(&self) -> String;
}

pub trait Builder<C: Clone>: Field + KeyAuths {
    fn build(cmd_block: SqlCmd<Self>) -> MaeRepo<Self> {
        MaeRepo::<Self> {
            cmd_block,
            whr_block: Self::keys(),
            repo_name: Self::repo_name(),
        }
    }
    fn repo_name() -> String;
}

pub trait KeyAuths: Field {
    fn keys() -> Vec<WhereCondition<Self>>;
}

// pub trait Interface<C: Clone, F: Field>: Builder<C> + Field {
//     fn insert_many(row: Vec<Self>) -> MaeRepo<Self> {
//         Self::build(SqlCmd::Insert(row))
//     }
//     fn select(cols: Vec<F>) -> MaeRepo<Self> {
//         Self::build(SqlCmd::Update(cols))
//     }
//     fn update_many(cols: Vec<F>) -> MaeRepo<Self> {
//         Self::build(SqlCmd::Update(cols))
//     }
// }
