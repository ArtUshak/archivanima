use std::{fmt::Display, ops::Deref, ops::DerefMut};

use rocket::{
    async_trait,
    data::{FromData, Outcome},
    form::{prelude::ErrorKind, Errors, Form, FromForm},
    http::Status,
    Data, Request,
};

use crate::utils::csrf_lib::{CsrfToken, VerificationFailure};

pub const COOKIE_NAME: &str = "csrf_token";

#[derive(Debug, Clone, Copy)]
pub struct CSRFError {}

unsafe impl Send for CSRFError {}

impl Display for CSRFError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSRF error")
    }
}

impl std::error::Error for CSRFError {}

pub struct CSRFProtectedForm<F> {
    form: Form<F>,
}

impl<F> Deref for CSRFProtectedForm<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.form
    }
}

impl<F> DerefMut for CSRFProtectedForm<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.form
    }
}

#[async_trait]
impl<'r, F> FromData<'r> for CSRFProtectedForm<F>
where
    F: CheckCSRF,
    F: FromForm<'r>,
{
    type Error = Errors<'r>;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> Outcome<'r, Self> {
        let token_outcome: rocket::request::Outcome<CsrfToken, ()> = req.guard().await;

        match token_outcome {
            rocket::request::Outcome::Success(csrf_token) => {
                let form_result: Outcome<Form<F>, _> = Form::from_data(req, data).await;
                match form_result {
                    Outcome::Success(form) => {
                        match form.check_csrf(&csrf_token) {
                            Ok(()) => Outcome::Success(CSRFProtectedForm { form }),
                            Err(_) => Outcome::Failure((
                                Status::Forbidden,
                                Errors::from(ErrorKind::Custom(Box::new(CSRFError {}))),
                            )), // TODO
                        }
                    }
                    Outcome::Failure(err) => Outcome::Failure(err),
                    Outcome::Forward(forward) => Outcome::Forward(forward),
                }
            }
            rocket::request::Outcome::Failure((status, ())) => {
                Outcome::Failure((status, Errors::new()))
            }
            rocket::request::Outcome::Forward(()) => Outcome::Forward(data),
        }
    }
}

pub trait CheckCSRF {
    fn check_csrf(&self, token: &CsrfToken) -> Result<(), VerificationFailure>;
}
