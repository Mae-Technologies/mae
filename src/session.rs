// The Actix cookie stores only the user id. The role snapshot is a Redis
// document, copied onto `Session.roles` by middleware. It is not written
// back into the cookie.
use actix_session::{Session as ActixSession, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{Ready, ready};
use std::ops::Deref;

use crate::grants::Grants;
use crate::route::response::ServiceError;

/// User id and row grants for one request.
///
/// `user` is the `sys_user` id. `None` means the call has no end user.
/// `roles` is [`Grants<T>`](crate::grants::Grants), and `T` is the snapshot
/// the application config chose. [`crate::context::ContextAccessor::session_user`]
/// reads `user`. A repo key reads `roles.snapshot()`.
///
/// [`SessionHandler`] only reads the Actix cookie. It does not carry grants.
/// Microservice middleware loads Redis and inserts this value. The cookie path
/// inserts an open snapshot, so those queries stay unrestricted.
///
/// # Examples
///
/// ```
/// use mae::grants::Grants;
/// use mae::session::Session;
///
/// let session = Session { user: Some(42), roles: Grants::<()>::open() };
/// assert_eq!(*session, Some(42));
/// assert!(session.roles.snapshot().is_none());
/// assert_eq!(format!("{}", session), "42");
/// ```
#[derive(Clone, Debug)]
pub struct Session<T> {
    /// `sys_user` id. `None` is an internal call with no end user.
    pub user: Option<i32>,
    /// Role snapshot for this request. Open adds no predicate.
    pub roles: Grants<T>
}

impl<T> Session<T> {
    pub fn session_or_err(&self) -> Result<i32, ServiceError> {
        match self.user {
            Some(id) => Ok(id),
            None => Err(ServiceError::Unauthorized)
        }
    }
}

impl<T> std::fmt::Display for Session<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.user {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "none")
        }
    }
}

impl<T> Deref for Session<T> {
    type Target = Option<i32>;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

/// Wrapper around Actix's [`actix_session::Session`] that enforces a typed
/// session key schema.
///
/// Only one session key is used — `"user_id"` — which stores the authenticated
/// user's integer ID.  All reads and writes go through this type to prevent
/// key-name typos and type mismatches.
#[derive(Clone)]
pub struct SessionHandler(ActixSession);

impl SessionHandler {
    const SESSION_KEY: &'static str = "user_id";

    pub fn new(session: ActixSession) -> Self {
        SessionHandler(session)
    }
    /// Renew the session cookie TTL without changing its contents.
    pub fn renew(&self) {
        self.0.renew();
    }

    /// Invalidate and remove the session from the store.
    pub fn purge(self) {
        self.0.purge();
    }

    /// Write the authenticated user's ID into the session store.
    ///
    /// Only the `user_id` integer is stored. Grants are not written into the cookie.
    /// Storing the full [`Session`] struct would produce `{"user_id":N}`, which
    /// cannot later be parsed as `i32`.
    pub fn insert_session<T>(&self, session_data: Session<T>) -> Result<(), SessionInsertError> {
        self.0.insert(Self::SESSION_KEY, session_data.user)
    }

    /// Read the user id from the store.
    ///
    /// Returns [`Session`] with an open snapshot. This store has no grants.
    /// Microservice middleware replaces that snapshot from Redis.
    ///
    /// Returns an empty user id when no session cookie is present or the session
    /// map has no `user_id`.
    pub fn get_session<T>(&self) -> Result<Session<T>, SessionGetError> {
        let session_map = self.0.entries();

        match session_map.is_empty() {
            true => Ok(Session { user: None, roles: Grants::open() }),
            false => {
                let user_id = session_map.get("user_id");
                match user_id {
                    Some(user_id) => {
                        let user_id = user_id.parse::<i32>();
                        match user_id {
                            Ok(user_id) => {
                                Ok(Session { user: Some(user_id), roles: Grants::open() })
                            }
                            Err(_) => Err(anyhow::anyhow!("Invalid user_id").into())
                        }
                    }
                    None => Ok(Session { user: None, roles: Grants::open() })
                }
            }
        }
    }
}

impl FromRequest for SessionHandler {
    type Error = <ActixSession as FromRequest>::Error;

    type Future = Ready<Result<SessionHandler, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(SessionHandler(req.get_session())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grants::Grants;
    use crate::route::response::ServiceError;
    use crate::testing::must::must_eq;

    #[test]
    fn session_or_err_returns_user_id() {
        let session = Session { user: Some(99), roles: Grants::<()>::open() };
        must_eq(session.session_or_err().expect("ok"), 99);
    }

    #[test]
    fn session_or_err_missing_is_unauthorized() {
        let session = Session { user: None, roles: Grants::<()>::open() };
        assert!(matches!(session.session_or_err(), Err(ServiceError::Unauthorized)));
    }

    #[test]
    fn display_formats_some_and_none() {
        must_eq(Session { user: Some(5), roles: Grants::<()>::open() }.to_string().as_str(), "5");
        must_eq(Session { user: None, roles: Grants::<()>::open() }.to_string().as_str(), "none");
    }

    #[test]
    fn deref_exposes_inner_option() {
        let session = Session { user: Some(12), roles: Grants::<()>::open() };
        must_eq(*session, Some(12));
    }

    #[test]
    fn roles_hold_the_snapshot() {
        let session = Session { user: Some(7), roles: Grants::defined(vec![4, 3]) };
        must_eq(session.user, Some(7));
        must_eq(session.roles.snapshot().cloned(), Some(vec![4, 3]));
    }
}
