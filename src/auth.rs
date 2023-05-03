use rocket::{
    async_trait,
    http::Status,
    request::{self, FromRequest, Outcome},
    Request, State,
};
use sqlx::{Pool, Postgres};

use crate::{
    app::db::{try_get_user, User},
    error,
};

pub const USERNAME_COOKIE_NAME: &str = "username";

#[derive(Clone, Debug)]
pub enum Authentication {
    Authenticated(User),
    Banned(User),
    Anonymous,
}

impl Authentication {
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated(_))
    }

    pub fn map<T, F>(&self, f: F) -> Option<T>
    where
        F: Fn(&User) -> T,
    {
        match self {
            Authentication::Authenticated(user) => Some(f(user)),
            Authentication::Banned(user) => Some(f(user)),
            Authentication::Anonymous => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.map(|user| user.is_admin).unwrap_or(false)
    }

    pub fn is_uploader(&self) -> bool {
        self.map(User::is_uploader).unwrap_or(false)
    }

    pub fn username(&self) -> Option<String> {
        self.map(|user| user.username.clone())
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for Authentication {
    type Error = error::Error;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let result: &request::Outcome<Self, Self::Error> = req
            .local_cache_async(async {
                let cookies = req.cookies();
                match cookies.get_private("username") {
                    Some(cookie) => {
                        let pool_state_result: Outcome<&State<Pool<Postgres>>, ()> =
                            req.guard().await;
                        match pool_state_result {
                            Outcome::Success(pool_state) => {
                                match try_get_user(cookie.value(), pool_state).await {
                                    Ok(Some(user)) if user.is_active => request::Outcome::Success(
                                        Authentication::Authenticated(user),
                                    ),
                                    Ok(Some(user_banned)) => request::Outcome::Success(
                                        Authentication::Banned(user_banned),
                                    ),
                                    Ok(_) => request::Outcome::Success(Authentication::Anonymous),
                                    Err(err) => request::Outcome::Failure((
                                        Status::InternalServerError,
                                        err,
                                    )),
                                }
                            }
                            Outcome::Failure((status, ())) => {
                                Outcome::Failure((status, error::Error::PoolNotFound))
                            }
                            Outcome::Forward(()) => Outcome::Forward(()),
                        }
                    }
                    None => request::Outcome::Success(Authentication::Anonymous),
                }
            })
            .await;

        result.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admin {}

#[async_trait]
impl<'r> FromRequest<'r> for Admin {
    type Error = error::Error;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        req.local_cache_async(async {
            match req.guard().await {
                Outcome::Success(Authentication::Authenticated(user)) if user.is_admin => {
                    Outcome::Success(Self {})
                }
                Outcome::Success(_) => {
                    Outcome::Failure((Status::Forbidden, error::Error::AccessDenied))
                }
                Outcome::Forward(()) => Outcome::Forward(()),
                Outcome::Failure(error) => Outcome::Failure(error),
            }
        })
        .await
        .clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uploader {}

#[async_trait]
impl<'r> FromRequest<'r> for Uploader {
    type Error = error::Error;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        req.local_cache_async(async {
            match req.guard().await {
                Outcome::Success(Authentication::Authenticated(user)) if user.is_uploader() => {
                    Outcome::Success(Self {})
                }
                Outcome::Success(_) => {
                    Outcome::Failure((Status::Forbidden, error::Error::AccessDenied))
                }
                Outcome::Forward(()) => Outcome::Forward(()),
                Outcome::Failure(error) => Outcome::Failure(error),
            }
        })
        .await
        .clone()
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = error::Error;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        req.local_cache_async(async {
            match req.guard().await {
                Outcome::Success(Authentication::Authenticated(user)) if user.is_admin => {
                    Outcome::Success(user)
                }
                Outcome::Success(_) => {
                    Outcome::Failure((Status::Forbidden, error::Error::AccessDenied))
                }
                Outcome::Forward(()) => Outcome::Forward(()),
                Outcome::Failure(error) => Outcome::Failure(error),
            }
        })
        .await
        .clone()
    }
}
