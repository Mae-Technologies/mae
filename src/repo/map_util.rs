use super::type_def::{ToField, ToInsertRow, ToPatch, ToUpdateRow};
use sqlx::Arguments;
use std::fmt::{Debug, Display};
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

pub fn concat_sql_parts(parts: Vec<(Vec<String>, Option<Vec<String>>)>) -> AsSqlParts {
    let mut cols = Vec::new();
    let mut binds: Option<Vec<String>> = None;

    for (c, b) in parts {
        cols.extend(c);

        if let Some(bv) = b {
            binds.get_or_insert_with(Vec::new).extend(bv);
        }
    }

    (cols, binds)
}

// SQL Statements
pub enum SqlStatement<I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch> {
    // TODO: I want an upsert() in here -> Insert, Update on conflict:
    // INSERT INTO users (email, name)
    // VALUES
    //   ('a@x.com', 'Alice'),
    //   ('b@x.com', 'Bob')
    // ON CONFLICT (email)
    // DO UPDATE SET
    //   name = EXCLUDED.name,
    //   updated_at = now()
    // RETURNING *;
    Select(Vec<F>),
    InsertOne(I),
    InsertMany(Vec<I>),
    Update(U),
    Patch(Vec<P>)
}

impl<I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch> BindArgs for SqlStatement<I, U, F, P> {
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
            Self::Patch(v) => v.iter().for_each(|f| f.bind(args))
        }
    }

    // Get the count of arg's that are to be bound
    fn bind_len(&self) -> usize {
        match self {
            Self::Update(v) => v.bind_len(),
            Self::InsertOne(v) => v.bind_len(),
            // NOTE: There are no bindings for select statements
            Self::Select(_) => 0,
            Self::InsertMany(v) => v.iter().map(|v| v.bind_len()).sum(),
            Self::Patch(v) => v.len()
        }
    }
}

impl<I: ToInsertRow, U: ToUpdateRow, F: ToField, P: ToPatch> Debug for SqlStatement<I, U, F, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlStatement::Select(_fields) => {
                // no fields to write
                std::fmt::Result::Ok(())
            }
            SqlStatement::InsertOne(row) => std::fmt::Debug::fmt(&row, f),
            SqlStatement::InsertMany(_row) => {
                todo!()
            }
            SqlStatement::Update(row) => std::fmt::Debug::fmt(&row, f),
            SqlStatement::Patch(fields) => {
                for (i, field) in fields.iter().enumerate() {
                    write!(f, "\n\t${} = ", i + 1)?;
                    std::fmt::Debug::fmt(field, f)?;
                }
                std::fmt::Result::Ok(())
            }
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
    IsNull
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
            Self::StringIs(v) => args.add(v.to_owned()),
            Self::StringIsNot(v) => args.add(v),
            Self::Gt(v) => args.add(v),
            Self::Gte(v) => args.add(v),
            Self::Lt(v) => args.add(v),
            Self::Lte(v) => args.add(v),
            Self::IsNull => Ok(())
        };
    }
    fn bind_len(&self) -> usize {
        match self {
            Self::IsNull => 0,
            _ => 1
        }
    }
}

impl std::fmt::Debug for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Equals(v) => write!(f, "{:?}", v),
            Self::NotEquals(v) => write!(f, "{:?}", v),
            Self::In(v) => write!(f, "{:?}", v),
            Self::NotIn(v) => write!(f, "{:?}", v),
            Self::Like(v) => write!(f, "{:?}", v),
            Self::NotLike(v) => write!(f, "{:?}", v),
            Self::Ilike(v) => write!(f, "{:?}", v),
            Self::NotIlike(v) => write!(f, "{:?}", v),
            Self::StringIs(v) => write!(f, "{:?}", v),
            Self::StringIsNot(v) => write!(f, "{:?}", v),
            Self::Gt(v) => write!(f, "{:?}", v),
            Self::Gte(v) => write!(f, "{:?}", v),
            Self::Lt(v) => write!(f, "{:?}", v),
            Self::Lte(v) => write!(f, "{:?}", v),
            Self::IsNull => Ok(())
        }
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
            Filter::IsNull => write!(f, "IS NULL")
        }
    }
}

// Filter / Where Operators
pub enum FilterOp<F: ToField> {
    And(F, Filter),
    Or(F, Filter),
    Begin(F, Filter)
}

impl<F: ToField> Display for FilterOp<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterOp::Begin(field, cond) => write!(f, "{} {}", field, cond),
            FilterOp::And(field, cond) => write!(f, "AND {} {}", field, cond),
            FilterOp::Or(field, cond) => write!(f, "OR {} {}", field, cond)
        }
    }
}

impl<F: ToField> Debug for FilterOp<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterOp::Begin(_, cond) => write!(f, "{:?}", cond),
            FilterOp::And(_, cond) => write!(f, "{:?}", cond),
            FilterOp::Or(_, cond) => write!(f, "{:?}", cond)
        }
    }
}

impl<F: ToField> BindArgs for FilterOp<F> {
    fn bind(&self, args: &mut sqlx::postgres::PgArguments) {
        match self {
            Self::Begin(_, w) => w.bind(args),
            Self::And(_, w) => w.bind(args),
            Self::Or(_, w) => w.bind(args)
        }
    }
    fn bind_len(&self) -> usize {
        match self {
            Self::Begin(_, w) => w.bind_len(),
            Self::And(_, w) => w.bind_len(),
            Self::Or(_, w) => w.bind_len()
        }
    }
}

// Static method to extract the Where block of the Sql Query. They will always be the same / have
// the same structure
pub fn sql_where<F: ToField>(
    w: &[FilterOp<F>],
    idx: usize,
    from_update_patch: Option<String>
) -> String {
    let update_batch_ref_table = match from_update_patch {
        Some(t) => format!("{t}."),
        None => "".into()
    };

    let mut f_idx = 0;
    let whr = w
        .iter()
        .map(|f| {
            let (kw, field_str) = match f {
                FilterOp::Begin(c, _) => ("", format!("{c}")),
                FilterOp::And(c, _) => ("AND ", format!("{c}")),
                FilterOp::Or(c, _) => ("OR ", format!("{c}"))
            };
            let filter_val = match f {
                FilterOp::Begin(_, v) | FilterOp::And(_, v) | FilterOp::Or(_, v) => v
            };
            match filter_val {
                Filter::IsNull => {
                    format!("\n\t{}{}{} IS NULL", kw, update_batch_ref_table, field_str)
                }
                v => {
                    f_idx += f.bind_len();
                    format!(
                        "\n\t{}{}{} {} ${}",
                        kw,
                        update_batch_ref_table,
                        field_str,
                        v,
                        f_idx + idx
                    )
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if !whr.is_empty() {
        return format!("\nWHERE{}", whr);
    }
    whr
}

#[cfg(all(test, feature = "test-utils"))]
mod tests {
    use super::*;
    use crate::repo::filter::{Filter, FilterOp};
    use crate::testing::must::must_be_true;

    /// Minimal field enum used only in unit tests so we don't need a real schema.
    #[derive(Clone, Copy)]
    enum TestField {
        Name,
        Age,
    }

    impl Display for TestField {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestField::Name => write!(f, "name"),
                TestField::Age => write!(f, "age"),
            }
        }
    }

    impl ToSqlParts for TestField {
        fn to_sql_parts(&self) -> AsSqlParts {
            (vec![format!("{}", self)], None)
        }
    }

    /// Regression test for #95: with a table alias, `FilterOp::And` must produce
    /// `AND alias.field = $N`, not `alias.AND field = $N`.
    #[test]
    fn sql_where_alias_precedes_keyword_not_field() {
        let filters = vec![
            FilterOp::Begin(TestField::Name, Filter::Equals(1)),
            FilterOp::And(TestField::Age, Filter::Equals(30)),
        ];

        let sql = sql_where(&filters, 0, Some("_x_".into()));

        must_be_true(sql.contains("AND _x_.age"));
        must_be_true(!sql.contains("_x_.AND"));
    }

    /// Same regression check for `FilterOp::Or`.
    #[test]
    fn sql_where_or_alias_precedes_keyword_not_field() {
        let filters = vec![
            FilterOp::Begin(TestField::Name, Filter::Equals(1)),
            FilterOp::Or(TestField::Age, Filter::Equals(30)),
        ];

        let sql = sql_where(&filters, 0, Some("_x_".into()));

        must_be_true(sql.contains("OR _x_.age"));
        must_be_true(!sql.contains("_x_.OR"));
    }

    /// `FilterOp::Begin` (no keyword) should still get the alias applied to the field.
    #[test]
    fn sql_where_begin_with_alias() {
        let filters = vec![FilterOp::Begin(TestField::Name, Filter::Equals(1))];

        let sql = sql_where(&filters, 0, Some("_x_".into()));

        must_be_true(sql.contains("_x_.name"));
    }

    /// `Filter::IsNull` with an alias and `FilterOp::And` must produce
    /// `AND alias.field IS NULL`, not `alias.AND field IS NULL`.
    #[test]
    fn sql_where_is_null_with_alias_and_keyword() {
        let filters = vec![
            FilterOp::Begin(TestField::Name, Filter::Equals(1)),
            FilterOp::And(TestField::Age, Filter::IsNull),
        ];

        let sql = sql_where(&filters, 0, Some("_x_".into()));

        must_be_true(sql.contains("AND _x_.age IS NULL"));
        must_be_true(!sql.contains("_x_.AND"));
    }

    /// Without an alias the output should be unchanged: keyword first, then field.
    #[test]
    fn sql_where_no_alias_and_keyword() {
        let filters = vec![
            FilterOp::Begin(TestField::Name, Filter::Equals(1)),
            FilterOp::And(TestField::Age, Filter::Equals(30)),
        ];

        let sql = sql_where(&filters, 0, None);

        must_be_true(sql.contains("AND age"));
    }
}
