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

use crate::route::response::ServiceError;

/// Authenticated session data attached to every guarded request.
///
/// Stored in Redis via [`SessionHandler`] and injected into request extensions by
/// the session middleware.  Access it in handlers via
/// `ReqData<Session>` or through a [`RequestContext`](crate::request_context::RequestContext).
///
/// # Examples
///
/// ```
/// use mae::session::Session;
///
/// let session = Session { user_id: 42 };
/// assert_eq!(*session, 42);           // Deref to i32
/// assert_eq!(format!("{}", session), "42"); // Display
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Session(pub Option<i32>);

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
    /// Only the `user_id` integer is stored; storing the full [`Session`] struct
    /// would produce `{"user_id":N}` which cannot later be parsed as `i32`.
    pub fn insert_session(&self, session_data: Session) -> Result<(), SessionInsertError> {
        self.0.insert(Self::SESSION_KEY, session_data.0)
    }

    /// Read the current session from the store.
    ///
    /// Returns `Ok(None)` when no session cookie is present or the session map
    /// is empty.  Returns `Ok(Some(Session))` when a valid `user_id` is found.
    pub fn get_session(&self) -> Result<Session, SessionGetError> {
        let session_map = self.0.entries();

        match session_map.is_empty() {
            true => Ok(Session(None)),
            false => {
                let user_id = session_map.get("user_id");
                match user_id {
                    Some(user_id) => {
                        let user_id = user_id.parse::<i32>();
                        match user_id {
                            Ok(user_id) => Ok(Session(Some(user_id))),
                            Err(_) => Err(anyhow::anyhow!("Invalid user_id").into())
                        }
                    }
                    None => Ok(Session(None))
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
