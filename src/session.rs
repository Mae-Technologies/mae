// NOTE: prior implementation of Sessions, entire user roles were posted to the session for ease
// within app, however, axtix_session does not like nested json types, or any json type other than
// Map<String, String> --- or HashMap. So the implementation for getting the roles will have to
// change, within the middleware. IE - get session.user_id => get auth.db.roles()
//  -- -- leaving these sections commented out until this is implemented.
use actix_session::{Session as ActixSession, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use serde::{Deserialize, Serialize};
use std::future::{Ready, ready};
use std::ops::Deref;

use crate::grants::Grants;
use crate::route::response::ServiceError;

/// User id and row grants for one request.
///
/// `.0` is the user id. `None` means the call has no end user. `.1` is the
/// [`Grants`] snapshot. [`crate::context::ContextAccessor::session_user`] reads
/// `.0`. A repo key reads `.1`.
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
/// let session = Session(Some(42), Grants::open());
/// assert_eq!(*session, Some(42));
/// assert_eq!(session.1.sys_clients(), None);
/// assert_eq!(format!("{}", session), "42");
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Session(
    /// User id. `None` is an internal call with no end user.
    pub Option<i32>,
    /// Companies this request may see. Open adds no predicate.
    pub Grants
);

impl Session {
    pub fn session_or_err(&self) -> Result<i32, ServiceError> {
        match self.0 {
            Some(id) => Ok(id),
            None => Err(ServiceError::Unauthorized)
        }
    }
}

impl std::fmt::Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "none")
        }
    }
}

impl Deref for Session {
    type Target = Option<i32>;

    fn deref(&self) -> &Self::Target {
        &self.0
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
    pub fn insert_session(&self, session_data: Session) -> Result<(), SessionInsertError> {
        self.0.insert(Self::SESSION_KEY, session_data.0)
    }

    /// Read the user id from the store.
    ///
    /// Returns [`Session`] with an open snapshot. This store has no grants.
    /// Microservice middleware replaces that snapshot from Redis.
    ///
    /// Returns an empty user id when no session cookie is present or the session
    /// map has no `user_id`.
    pub fn get_session(&self) -> Result<Session, SessionGetError> {
        let session_map = self.0.entries();

        match session_map.is_empty() {
            true => Ok(Session(None, Grants::open())),
            false => {
                let user_id = session_map.get("user_id");
                match user_id {
                    Some(user_id) => {
                        let user_id = user_id.parse::<i32>();
                        match user_id {
                            Ok(user_id) => Ok(Session(Some(user_id), Grants::open())),
                            Err(_) => Err(anyhow::anyhow!("Invalid user_id").into())
                        }
                    }
                    None => Ok(Session(None, Grants::open()))
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
        let session = Session(Some(99), Grants::open());
        must_eq(session.session_or_err().expect("ok"), 99);
    }

    #[test]
    fn session_or_err_missing_is_unauthorized() {
        let session = Session(None, Grants::open());
        assert!(matches!(session.session_or_err(), Err(ServiceError::Unauthorized)));
    }

    #[test]
    fn display_formats_some_and_none() {
        must_eq(Session(Some(5), Grants::open()).to_string().as_str(), "5");
        must_eq(Session(None, Grants::open()).to_string().as_str(), "none");
    }

    #[test]
    fn deref_exposes_inner_option() {
        let session = Session(Some(12), Grants::open());
        must_eq(*session, Some(12));
    }

    #[test]
    fn grants_are_the_second_finger() {
        let session = Session(Some(7), Grants::companies(vec![4, 3, 3]));
        must_eq(session.0, Some(7));
        must_eq(session.1.sys_clients(), Some(vec![3, 4]));
    }
}
