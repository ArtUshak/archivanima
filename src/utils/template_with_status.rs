use std::io::Cursor;

use askama::Template;
use askama_rocket::Responder;
use rocket::{
    http::{Header, Status},
    response, Request, Response,
};

pub struct TemplateForbidden<T: Template> {
    pub template: T,
}

impl<'r, 'o: 'r, T: Template> Responder<'r, 'o> for TemplateForbidden<T> {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'o> {
        let response = self
            .template
            .render()
            .map_err(|_| Status::InternalServerError)?;
        Response::build()
            .status(Status::Forbidden)
            .header(Header::new("content-type", T::MIME_TYPE))
            .sized_body(response.len(), Cursor::new(response))
            .ok()
    }
}

pub struct TemplateUnavailableForLegal<T: Template> {
    pub template: T,
}

impl<'r, 'o: 'r, T: Template> Responder<'r, 'o> for TemplateUnavailableForLegal<T> {
    fn respond_to(self, _request: &'r Request<'_>) -> response::Result<'o> {
        let response = self
            .template
            .render()
            .map_err(|_| Status::InternalServerError)?;
        Response::build()
            .status(Status::UnavailableForLegalReasons)
            .header(Header::new("content-type", T::MIME_TYPE))
            .sized_body(response.len(), Cursor::new(response))
            .ok()
    }
}
