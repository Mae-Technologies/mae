use sqlx::Arguments;
use sqlx::postgres::PgArguments;
use std::fmt::{self, Display};

use crate::request_context::ContextAccessor;

pub struct MaeRepo<F: Display + ToSql> {
    cmd_block: SqlCmd<F>,
    whr_block: Vec<WhereCondition<F>>,
}

pub enum SqlCmd<F: ToSql> {
    Select(Vec<F>),
    Insert(Vec<F>),
    Update(Vec<F>),
}

impl<F: ToSql + Display> SqlCmd<F> {
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

    fn sql(&self, repo_name: String, whr_block: &Vec<WhereCondition<F>>) -> String {
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

impl<F: Display + ToSql + BindArgs> BindArgs for SqlCmd<F> {
    fn bind(&self, mut args: &mut PgArguments) {
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

pub enum WhereCondition<F: Display> {
    And(F, Where),
    Or(F, Where),
    Begin(F, Where),
}

impl<F: Display> Display for WhereCondition<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhereCondition::Begin(field, cond) => write!(f, "{} {}", field, cond),
            WhereCondition::And(field, cond) => write!(f, "{} AND {} {}", field, field, cond),
            WhereCondition::Or(field, cond) => write!(f, "{} OR {} {}", field, field, cond),
        }
    }
}

impl<F: Display> BindWhere for WhereCondition<F> {
    fn bind(&self, args: &mut PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args),
        }
    }
}

fn sql_where<F: Display>(w: &Vec<WhereCondition<F>>) -> String {
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

impl Where {
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

trait BindWhere {
    fn bind(&self, args: &mut PgArguments);
}

pub trait ToSql {
    fn sql_insert(&self) -> String;
    fn sql_update(&self) -> String;
    fn sql_select(&self) -> String;
}

pub trait Builder<
    F: BindArgs + Display + ToSql,
    R: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    C: ContextAccessor + Clone + Sync,
>
{
    fn build(sys_client: i32, cmd_block: SqlCmd<F>) -> MaeRepo<F> {
        let whr_block = vec![WhereCondition::Begin(
            Self::get_sys_client_field(),
            Where::Equals(sys_client),
        )];
        MaeRepo::<F> {
            cmd_block,
            whr_block,
        }
    }
    fn cmd_block(&self) -> SqlCmd<F>;
    fn repo_name() -> String;
    fn whr_block(&self) -> Vec<WhereCondition<F>>;
    fn args(&self) -> PgArguments {
        let mut args = PgArguments::default();
        // NOTE: we need to bind the args before the where statement.
        self.cmd_block().bind(&mut args);
        for w in self.whr_block().iter() {
            w.bind(&mut args);
        }
        args
    }
    fn execute(
        &self,
        ctx: &C,
    ) -> impl std::future::Future<Output = Result<Vec<R>, anyhow::Error>> + Send
    where
        Self: Sync,
    {
        async move {
            let sql = &self.cmd_block().sql(Self::repo_name(), &self.whr_block());
            let query = sqlx::query_as_with(sql, self.args());
            Ok(query.fetch_all(ctx.db_pool()).await?)
        }
    }
    fn get_sys_client_field() -> F;
}
