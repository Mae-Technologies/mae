// NOTE: prior implementation of Sessions, entire user roles were posted to the session for ease
// within app, however, axtix_session does not like nested json types, or any json type other than
// Map<String, String> --- or HashMap. So the implementation for getting the roles will have to
// change, within the middleware. IE - get session.user_id => get auth.db.roles()
//  -- -- leaving these sections commented out until this is implemented.
use actix_session::{Session as ActixSession, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use serde::{Deserialize, Serialize};
// use serde_json::Value;
use std::future::{Ready, ready};
use std::ops::Deref;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Session {
    // pub user_data: UserData,
    pub user_id: i32,
    // pub session_id: String,
}

// #[derive(Clone, Serialize, Deserialize, Debug)]
// pub struct UserData {
//     pub id: i32,
//     pub status: String,
//     pub roles: UserRoles,
// }
//
// #[derive(Clone, Serialize, Deserialize, Debug)]
// pub struct UserRoles {
//     pub role: String,
//     pub has_limits: bool,
//     pub limit_details: Value,
//     pub tags: Vec<String>,
// }

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

pub struct TypedSession(ActixSession);

impl TypedSession {
    const SESSION_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn purge(self) {
        self.0.purge();
    }

    pub fn insert_session(&self, session_data: Session) -> Result<(), SessionInsertError> {
        self.0.insert(Self::SESSION_KEY, session_data)
    }

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
                            Err(_) => Ok(None),
                        }
                    }
                    None => Ok(None),
                }
            }
        }
        // match self.0.get::<Session>(Self::SESSION_KEY) {
        //     Ok(Some(session)) => Ok(Some(session)),
        //     Ok(None) => Ok(None),
        //     Err(e) => Err(e),
        // }
    }
}

impl FromRequest for TypedSession {
    type Error = <ActixSession as FromRequest>::Error;

    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
