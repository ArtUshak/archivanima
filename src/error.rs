use std::fmt::Display;

use argon2::password_hash;
use askama_rocket::Responder;
use log::debug;
use rocket::{http::Status, response, Request};

#[derive(Clone, Debug)]
pub enum Error {
    Sqlx(String),
    PasswordHash(password_hash::Error),
    PoolNotFound,
    Rocket(String),
    AccessDenied,
    DoesNotExist,
    InvalidPagination,
    PageDoesNotExist,
    UnknownError,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Error::Sqlx(_) => "DB error",
                Error::PasswordHash(_) => "Password hash error",
                Error::PoolNotFound => "DB error",
                Error::Rocket(_) => "Rocket error",
                Error::AccessDenied => "Access denied",
                Error::DoesNotExist => "Object does not exist",
                Error::InvalidPagination => "Invalid pagination param",
                Error::PageDoesNotExist => "Page does not exist",
                Error::UnknownError => "Unknown error",
            }
        )
    }
}

impl std::error::Error for Error {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }

    fn description(&self) -> &str {
        match self {
            Error::Sqlx(_) => "DB error",
            Error::PasswordHash(_) => "Password hash error",
            Error::PoolNotFound => "DB error",
            Error::Rocket(_) => "Rocket error",
            Error::AccessDenied => "Access denied",
            Error::DoesNotExist => "Object does not exist",
            Error::InvalidPagination => "Invalid pagination param",
            Error::PageDoesNotExist => "Page does not exist",
            Error::UnknownError => "Unknown error",
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Self::Sqlx(value.to_string())
    }
}

impl From<password_hash::Error> for Error {
    fn from(value: password_hash::Error) -> Self {
        Self::PasswordHash(value)
    }
}

impl From<rocket::Error> for Error {
    fn from(value: rocket::Error) -> Self {
        Self::Rocket(value.to_string())
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'o> {
        debug!("Processing error {:?}", &self);
        let status_code = match self {
            Error::Sqlx(_) => Status::InternalServerError,
            Error::PasswordHash(_) => Status::InternalServerError,
            Error::PoolNotFound => Status::InternalServerError,
            Error::Rocket(_) => Status::InternalServerError,
            Error::AccessDenied => Status::Forbidden,
            Error::DoesNotExist => Status::NotFound,
            Error::InvalidPagination => Status::UnprocessableEntity,
            Error::PageDoesNotExist => Status::NotFound,
            Error::UnknownError => Status::InternalServerError,
        };
        if status_code.code % 100 == 5 {
            log::error!("Internal server error: {:?}", &self);
        }
        Err(status_code)
    }
}
