use super::map_util::{BindArgs, ToSql};
use crate::request_context::ContextAccessor;
use std::fmt::Display;
// /////
// TYPES
//  ////

//  SOMETHING THAT WILL HAVE OUR CONTEXT -> C
pub trait Context: ContextAccessor + Unpin + Send {}
impl<C> Context for C where C: ContextAccessor + Unpin + Send {}

// SOMETHING THAT WILL CONVERT TO A ROW -> T
pub trait ToRow: ToSql + BindArgs {}
impl<R> ToRow for R where R: ToSql + BindArgs {}

// SOMETHING THAT WILL CONVERT TO A FIELD -> F
pub trait ToField: Display {}
impl<F> ToField for F where F: Display {}

// SOMETHING THAT WILL CONVER TO  A FIELD<T> -> P
pub trait ToPatch: ToSql + BindArgs {}
impl<P> ToPatch for P where P: ToSql + BindArgs {}

// SOMETHING THAT AN SQL ROW CAN BE CONVERTED INTO -> A
pub trait QueryAs: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}
impl<A> QueryAs for A where A: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send {}
