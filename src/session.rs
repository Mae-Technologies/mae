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

/// Authenticated session data attached to every guarded request.
///
/// Stored in Redis via [`TypedSession`] and injected into request extensions by
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
pub struct Session {
    pub user_id: i32
}

impl std::fmt::Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.user_id.fmt(f)
    }
}

impl Deref for Session {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.user_id
    }
}

/// Wrapper around Actix's [`actix_session::Session`] that enforces a typed
/// session key schema.
///
/// Only one session key is used — `"user_id"` — which stores the authenticated
/// user's integer ID.  All reads and writes go through this type to prevent
/// key-name typos and type mismatches.
pub struct TypedSession(ActixSession);

impl TypedSession {
    const SESSION_KEY: &'static str = "user_id";

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
        self.0.insert(Self::SESSION_KEY, session_data.user_id)
    }

    /// Read the current session from the store.
    ///
    /// Returns `Ok(None)` when no session cookie is present or the session map
    /// is empty.  Returns `Ok(Some(Session))` when a valid `user_id` is found.
    pub fn get_session(&self) -> Result<Option<Session>, SessionGetError> {
        let session_map = self.0.entries();

        match session_map.is_empty() {
            true => Ok(None),
            false => {
                let user_id = session_map.get("user_id");
                match user_id {
                    Some(user_id) => {
                        let user_id = user_id.parse::<i32>();
                        match user_id {
                            Ok(user_id) => Ok(Some(Session { user_id })),
                            Err(_) => Ok(None)
                        }
                    }
                    None => Ok(None)
                }
            }
        }
    }
}

impl FromRequest for TypedSession {
    type Error = <ActixSession as FromRequest>::Error;

    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
