//! Row grants for one user, stored in the shared Redis.
//!
//! The application config names the snapshot type ([`GrantConfig::Snapshot`]).
//! That type is the `sys_user` and `sys_roles` document from statbook_models.
//! Mae does not own those schemas and does not read company ids out of them.
//! [`GrantWriter`] writes the snapshot at login and whenever roles change.
//! Request middleware holds a [`GrantStore`], which can only read. It copies
//! the snapshot onto [`crate::session::Session::roles`]. There is no setter.
//! Handlers cannot write Redis through the store. Allow-lists stay off the wire.
//!
//! [`Grants::open`] is no snapshot: an internal call with no end user.
//! [`Grants::defined`] is the document `T`. A repo key reads `T` and builds
//! its own predicate.

use redis::aio::ConnectionManager;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

/// Redis key prefix. Distinct from the Actix session keys.
pub const KEY_PREFIX: &str = "statbook:grants:";

/// Lifetime of a snapshot when the writer does not refresh it.
/// The API writes again on login and on every role change.
pub const DEFAULT_TTL: Duration = Duration::from_secs(8 * 60 * 60 + 30 * 60);

/// Chooses the Redis document for one service. Implement this on the
/// application config (`AppContext`), not on a handler.
///
/// `Snapshot` is the `sys_user` plus `sys_roles` document defined in
/// statbook_models. Mae stores and returns that value. It does not interpret it.
pub trait GrantConfig {
    type Snapshot: Clone + Send + Sync + 'static + Serialize + DeserializeOwned;
}

impl GrantConfig for () {
    type Snapshot = ();
}

/// Role snapshot for one request. `T` comes from [`GrantConfig::Snapshot`].
///
/// The field is private. [`Grants::open`] and [`Grants::defined`] are the
/// constructors. [`Grants::snapshot`] reads `T`. A repo key uses that value
/// to build its predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grants<T> {
    snapshot: Option<T>
}

impl<T> Grants<T> {
    /// No snapshot. Key filters add no predicate.
    pub fn open() -> Self {
        Self { snapshot: None }
    }

    /// The application snapshot, as stored in Redis.
    pub fn defined(snapshot: T) -> Self {
        Self { snapshot: Some(snapshot) }
    }

    /// `None` when the call has no end user. `Some` is the document `T`.
    pub fn snapshot(&self) -> Option<&T> {
        self.snapshot.as_ref()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GrantsError {
    #[error("grant store unavailable")]
    Unavailable,
    #[error("grant snapshot is not readable")]
    Unreadable
}

/// What a service does with the snapshot for one internal call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantDecision<T> {
    /// No end user on the call. Queries stay unrestricted.
    Open,
    /// Apply this snapshot. The repo key reads `T`.
    Apply(Grants<T>),
    /// A user id was presented and there is no usable snapshot.
    Deny,
    /// Redis could not answer. Do not fall open.
    Unavailable
}

/// `None` user id is open even when the lookup failed. A user id never is.
pub fn resolve_grants<T>(
    user_id: Option<i32>,
    loaded: Result<Option<T>, GrantsError>
) -> GrantDecision<T> {
    let Some(_) = user_id else {
        return GrantDecision::Open;
    };
    match loaded {
        Ok(Some(snapshot)) => GrantDecision::Apply(Grants::defined(snapshot)),
        Ok(None) | Err(GrantsError::Unreadable) => GrantDecision::Deny,
        Err(GrantsError::Unavailable) => GrantDecision::Unavailable
    }
}

pub fn key(user_id: i32) -> String {
    format!("{KEY_PREFIX}{user_id}")
}

/// The Redis value is `T`. Mae does not require a particular field.
fn parse_snapshot<T: DeserializeOwned>(body: &str) -> Result<T, GrantsError> {
    serde_json::from_str(body).map_err(|_| GrantsError::Unreadable)
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

    pub async fn get<T: DeserializeOwned>(&self, user_id: i32) -> Result<Option<T>, GrantsError> {
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

    pub async fn put<T: Serialize>(&self, user_id: i32, snapshot: &T) -> Result<(), GrantsError> {
        self.put_ttl(user_id, snapshot, DEFAULT_TTL).await
    }

    pub async fn put_ttl<T: Serialize>(
        &self,
        user_id: i32,
        snapshot: &T,
        ttl: Duration
    ) -> Result<(), GrantsError> {
        let body = serde_json::to_string(snapshot).map_err(|_| GrantsError::Unreadable)?;
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
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Sample {
        sys_client: Vec<i32>
    }

    #[test]
    fn open_has_no_snapshot_and_defined_returns_it() {
        let open = Grants::<Sample>::open();
        must_be_true(open.snapshot().is_none());
        let defined = Grants::defined(Sample { sys_client: vec![4, 3] });
        must_eq(defined.snapshot().map(|row| row.sys_client.clone()), Some(vec![4, 3]));
    }

    #[test]
    fn snapshot_json_is_the_document_itself() {
        let raw = serde_json::to_string(&Sample { sys_client: vec![3, 4] }).expect("json");
        let parsed: Result<Sample, _> = parse_snapshot(&raw);
        must_eq(parsed, Ok(Sample { sys_client: vec![3, 4] }));
        let extra: Result<Sample, _> =
            parse_snapshot("{\"sys_client\":[3,4],\"operation\":[1,11]}");
        must_eq(extra, Ok(Sample { sys_client: vec![3, 4] }));
        let missing: Result<Sample, _> = parse_snapshot("{}");
        must_eq(missing, Err(GrantsError::Unreadable));
        let junk: Result<Sample, _> = parse_snapshot("not-json");
        must_eq(junk, Err(GrantsError::Unreadable));
    }

    #[test]
    fn key_is_the_user_id() {
        must_eq(key(3).as_str(), "statbook:grants:3");
    }

    #[test]
    fn resolve_fails_closed_for_a_user_and_stays_open_without_one() {
        let snapshot = Sample { sys_client: vec![3] };
        must_eq(resolve_grants::<Sample>(None, Err(GrantsError::Unavailable)), GrantDecision::Open);
        must_eq(
            resolve_grants(Some(7), Ok(Some(snapshot.clone()))),
            GrantDecision::Apply(Grants::defined(snapshot))
        );
        must_eq(resolve_grants::<Sample>(Some(7), Ok(None)), GrantDecision::Deny);
        must_eq(
            resolve_grants::<Sample>(Some(7), Err(GrantsError::Unreadable)),
            GrantDecision::Deny
        );
        must_eq(
            resolve_grants::<Sample>(Some(7), Err(GrantsError::Unavailable)),
            GrantDecision::Unavailable
        );
    }
}
