use super::type_def::{ToField, ToPatch, ToRow};
use crate::request_context::ContextAccessor;
use num::iter;
use sqlx::Arguments;
use std::{fmt::Display, ops::Deref};
// /////
// INTERNAL: CONVERT THE BUILDER PARTS TO SQL
// /////

// There are two interfaces to all the SQL parts:

//  - BindArgs
//      If there are arguments, they need to be safely inserted into the SQL Query with PgArguments
pub trait BindArgs {
    // TODO: if any of these are to panic, this method should return a Result
    fn bind(&self, args: &mut sqlx::postgres::PgArguments);
    fn bind_len(&self) -> usize;
}

//  - ToSqlParts
//      If there are column representations inside the types, they need to be extracted.
//      This is done with the Dispay impl
pub type AsSqlParts = (Vec<String>, Option<Vec<String>>);
pub trait ToSqlParts {
    fn to_sql_parts(&self) -> AsSqlParts;
}

impl<R: ToRow, F: ToField, P: ToPatch> ToSqlParts for SqlStatement<R, F, P> {
    fn to_sql_parts(&self) -> AsSqlParts {
        todo!()
        // TODO: This has to look something like this for an update many:
        //UPDATE users u
        // SET
        //     name = v.name,
        //     age  = v.age
        // FROM (
        //     VALUES
        //         (1, 'Alice', 30),
        //         (2, 'Bob',   25),
        //         (3, 'Carol', 40)
        // ) AS v(id, name, age)
        // WHERE u.id = v.id;
    }
}

// SQL Statements
pub enum SqlStatement<R: ToRow, F: ToField, P: ToPatch> {
    Select(Vec<F>),
    InsertOne(R),
    InsertMany(Vec<R>),
    Update(R),
    Patch(Vec<P>),
}

impl<R: ToRow, F: ToField, P: ToPatch> BindArgs for SqlStatement<R, F, P> {
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
pub enum Filter {
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

impl BindArgs for Filter {
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

impl Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Filter::Equals(_) => write!(f, "="),
            Filter::NotEquals(_) => write!(f, "!="),
            Filter::In(_) => write!(f, "IN"),
            Filter::NotIn(_) => write!(f, "NOT IN"),
            Filter::Like(_) => write!(f, "LIKE"),
            Filter::NotLike(_) => write!(f, "NOT LIKE"),
            Filter::Ilike(_) => write!(f, "ILIKE"),
            Filter::NotIlike(_) => write!(f, "NOT ILIKE"),
            Filter::StringIs(_) => write!(f, "="),
            Filter::StringIsNot(_) => write!(f, "!="),
            Filter::Gt(_) => write!(f, ">"),
            Filter::Gte(_) => write!(f, ">="),
            Filter::Lt(_) => write!(f, "<"),
            Filter::Lte(_) => write!(f, "<="),
            Filter::IsNull => write!(f, "IS NULL"),
        }
    }
}

// Filter / Where Operators
pub enum FilterOp<F: ToField> {
    And(F, Filter),
    Or(F, Filter),
    Begin(F, Filter),
}

impl<F: ToField> Display for FilterOp<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterOp::Begin(field, cond) => write!(f, "{} {}", field, cond),
            FilterOp::And(field, cond) => write!(f, "AND {} {}", field, cond),
            FilterOp::Or(field, cond) => write!(f, "OR {} {}", field, cond),
        }
    }
}

impl<F: ToField> BindArgs for FilterOp<F> {
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
pub fn sql_where<F: ToField>(w: &Vec<FilterOp<F>>, idx: usize) -> String {
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
