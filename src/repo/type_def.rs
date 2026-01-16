use super::map_util::{BindArgs, ToSqlParts};
use crate::request_context::ContextAccessor;
use std::fmt::Display;
// /////
// TYPES
//  ////

//  SOMETHING THAT WILL HAVE OUR CONTEXT -> C
pub trait Context: ContextAccessor + Unpin + Send + Sync {}
impl<C,> Context for C where C: ContextAccessor + Unpin + Send + Sync {}

// SOMETHING THAT WILL CONVERT TO A ROW -> T
pub trait ToRow: ToSqlParts + BindArgs {}
impl<R,> ToRow for R where R: ToSqlParts + BindArgs {}

// SOMETHING THAT WILL CONVERT TO A FIELD -> F
pub trait ToField: ToSqlParts + Display {}
impl<F,> ToField for F where F: ToSqlParts + Display {}

// SOMETHING THAT WILL CONVER TO  A FIELD<T> -> P
pub trait ToPatch: ToSqlParts + BindArgs {}
impl<P,> ToPatch for P where P: ToSqlParts + BindArgs {}

// SOMETHING THAT AN SQL ROW CAN BE CONVERTED INTO -> A
pub trait QueryAs: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow,> + Unpin + Send {}
impl<A,> QueryAs for A where A: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow,> + Unpin + Send {}
