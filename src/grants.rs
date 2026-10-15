//! Row grants for one user, stored in the shared Redis.
//!
//! [`GrantWriter`] writes the snapshot at login and whenever roles change.
//! Request middleware holds a [`GrantStore`], which can only read. It copies
//! the snapshot onto the request. Handlers cannot replace that copy, and they
//! cannot write Redis through the store. Allow-lists stay off the wire. A
//! later collection (operations, projects) is another field on this document.
//!
//! `sys_client: null` is unrestricted. `sys_client: []` matches nobody.

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Redis key prefix. Distinct from the Actix session keys.
pub const KEY_PREFIX: &str = "statbook:grants:";

/// Lifetime of a snapshot when the writer does not refresh it.
/// The API writes again on login and on every role change.
pub const DEFAULT_TTL: Duration = Duration::from_secs(8 * 60 * 60 + 30 * 60);

/// Companies the user may see. Further collections can be added with
/// `#[serde(default)]` so an older snapshot still parses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    pub sys_client: Option<Vec<i32>>
}

impl Grants {
    /// Super, or any other caller who is not limited by company.
    pub fn open() -> Self {
        Self { sys_client: None }
    }

    /// These companies only. Ids are sorted and de-duplicated.
    pub fn companies(ids: impl Into<Vec<i32>>) -> Self {
        Self { sys_client: Some(canonical_ids(ids.into())) }
    }

    pub fn sys_clients(&self) -> Option<Vec<i32>> {
        self.sys_client.clone()
    }
}

/// Snapshot the microservice middleware places on the request.
/// Absent means the caller has no end user. Crate-private so a handler or a
/// downstream middleware cannot forge a wider snapshot.
#[derive(Clone, Debug)]
pub(crate) struct RequestGrants(pub(crate) Grants);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GrantsError {
    #[error("grant store unavailable")]
    Unavailable,
    #[error("grant snapshot is not readable")]
    Unreadable
}

/// What a service does with the snapshot for one internal call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantDecision {
    /// No end user on the call. Queries stay unrestricted.
    Open,
    /// Apply this snapshot. An open snapshot adds no company predicate.
    Apply(Grants),
    /// A user id was presented and there is no usable snapshot.
    Deny,
    /// Redis could not answer. Do not fall open.
    Unavailable
}

/// `None` user id is open even when the lookup failed. A user id never is.
pub fn resolve_grants(
    user_id: Option<i32>,
    loaded: Result<Option<Grants>, GrantsError>
) -> GrantDecision {
    let Some(_) = user_id else {
        return GrantDecision::Open;
    };
    match loaded {
        Ok(Some(grants)) => GrantDecision::Apply(grants),
        Ok(None) | Err(GrantsError::Unreadable) => GrantDecision::Deny,
        Err(GrantsError::Unavailable) => GrantDecision::Unavailable
    }
}

pub fn key(user_id: i32) -> String {
    format!("{KEY_PREFIX}{user_id}")
}

fn canonical_ids(mut ids: Vec<i32>) -> Vec<i32> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn store_form(grants: &Grants) -> Grants {
    match &grants.sys_client {
        Some(ids) => Grants::companies(ids.clone()),
        None => Grants::open()
    }
}

/// An absent `sys_client` key deserializes as `None`, which means unrestricted.
/// The key has to be present. `null` is unrestricted. `[]` matches nobody.
fn parse_snapshot(body: &str) -> Result<Grants, GrantsError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| GrantsError::Unreadable)?;
    let obj = value.as_object().ok_or(GrantsError::Unreadable)?;
    if !obj.contains_key("sys_client") {
        return Err(GrantsError::Unreadable);
    }
    serde_json::from_value(value).map_err(|_| GrantsError::Unreadable)
}

async fn connect_manager(redis_uri: &str) -> Result<ConnectionManager, GrantsError> {
    let client = redis::Client::open(redis_uri).map_err(|err| {
        tracing::warn!(error = %err, "grant store could not open Redis");
        GrantsError::Unavailable
    })?;
    ConnectionManager::new(client).await.map_err(|err| {
        tracing::warn!(error = %err, "grant store could not connect to Redis");
        GrantsError::Unavailable
    })
}

/// Read-only Redis client for request middleware. Clone is cheap.
/// One store serves every worker. This type cannot write or delete a snapshot.
#[derive(Clone)]
pub struct GrantStore {
    manager: ConnectionManager
}

impl GrantStore {
    /// Connect to the same Redis the session store uses.
    pub async fn connect(redis_uri: &str) -> Result<Self, GrantsError> {
        Ok(Self { manager: connect_manager(redis_uri).await? })
    }

    pub async fn get(&self, user_id: i32) -> Result<Option<Grants>, GrantsError> {
        let mut conn = self.manager.clone();
        let raw: Option<String> =
            redis::cmd("GET").arg(key(user_id)).query_async(&mut conn).await.map_err(|err| {
                tracing::warn!(error = %err, user_id, "grant snapshot could not be read");
                GrantsError::Unavailable
            })?;
        match raw {
            None => Ok(None),
            Some(body) => parse_snapshot(&body)
                .inspect_err(|_| {
                    tracing::warn!(user_id, "grant snapshot is not readable");
                })
                .map(Some)
        }
    }
}

/// Writes a snapshot at login and on role change.
/// Request middleware does not hold this type. Do not put it in handler app data.
#[derive(Clone)]
pub struct GrantWriter {
    manager: ConnectionManager
}

impl GrantWriter {
    /// Connect to the same Redis the session store uses.
    pub async fn connect(redis_uri: &str) -> Result<Self, GrantsError> {
        Ok(Self { manager: connect_manager(redis_uri).await? })
    }

    pub async fn put(&self, user_id: i32, grants: &Grants) -> Result<(), GrantsError> {
        self.put_ttl(user_id, grants, DEFAULT_TTL).await
    }

    pub async fn put_ttl(
        &self,
        user_id: i32,
        grants: &Grants,
        ttl: Duration
    ) -> Result<(), GrantsError> {
        let body =
            serde_json::to_string(&store_form(grants)).map_err(|_| GrantsError::Unreadable)?;
        let mut conn = self.manager.clone();
        redis::cmd("SET")
            .arg(key(user_id))
            .arg(body)
            .arg("EX")
            .arg(ttl.as_secs().max(1))
            .query_async::<()>(&mut conn)
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, user_id, "grant snapshot could not be written");
                GrantsError::Unavailable
            })
    }

    pub async fn delete(&self, user_id: i32) -> Result<(), GrantsError> {
        let mut conn = self.manager.clone();
        redis::cmd("DEL").arg(key(user_id)).query_async::<()>(&mut conn).await.map_err(|err| {
            tracing::warn!(error = %err, user_id, "grant snapshot could not be deleted");
            GrantsError::Unavailable
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GrantDecision, Grants, GrantsError, key, parse_snapshot, resolve_grants};
    use crate::testing::must::{must_be_true, must_eq};

    #[test]
    fn companies_are_sorted_and_unique() {
        must_eq(Grants::companies(vec![4, 3, 3]).sys_client, Some(vec![3, 4]));
        must_be_true(Grants::open().sys_client.is_none());
        must_be_true(Grants::companies(vec![]).sys_client == Some(vec![]));
    }

    #[test]
    fn open_snapshot_writes_null_and_a_partial_document_does_not_parse() {
        let raw = serde_json::to_string(&Grants::open()).expect("json");
        must_eq(raw.as_str(), "{\"sys_client\":null}");
        must_eq(parse_snapshot(&raw), Ok(Grants::open()));
        must_eq(parse_snapshot("{}"), Err(GrantsError::Unreadable));
        must_eq(parse_snapshot("not-json"), Err(GrantsError::Unreadable));
        let parsed = parse_snapshot("{\"sys_client\":[3,4],\"operation\":[1,11]}")
            .expect("forward compatible");
        must_eq(parsed.sys_client, Some(vec![3, 4]));
    }

    #[test]
    fn key_is_the_user_id() {
        must_eq(key(3).as_str(), "statbook:grants:3");
    }

    #[test]
    fn resolve_fails_closed_for_a_user_and_stays_open_without_one() {
        let companies = Grants::companies(vec![3]);
        must_eq(resolve_grants(None, Err(GrantsError::Unavailable)), GrantDecision::Open);
        must_eq(
            resolve_grants(Some(7), Ok(Some(companies.clone()))),
            GrantDecision::Apply(companies)
        );
        must_eq(
            resolve_grants(Some(7), Ok(Some(Grants::open()))),
            GrantDecision::Apply(Grants::open())
        );
        must_eq(resolve_grants(Some(7), Ok(None)), GrantDecision::Deny);
        must_eq(resolve_grants(Some(7), Err(GrantsError::Unreadable)), GrantDecision::Deny);
        must_eq(resolve_grants(Some(7), Err(GrantsError::Unavailable)), GrantDecision::Unavailable);
    }
}
