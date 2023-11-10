use rocket::{
    async_trait,
    http::{hyper::header::CONTENT_RANGE, Status},
    request::{self, FromRequest, Request},
    Either,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentRange(
    pub Either<http_content_range::ContentRangeBytes, http_content_range::ContentRangeUnbound>,
);

#[async_trait]
impl<'r> FromRequest<'r> for ContentRange {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let header = req.headers().get_one(CONTENT_RANGE.as_str());
        match header {
            Some(header_real) => match http_content_range::ContentRange::parse(header_real) {
                http_content_range::ContentRange::Bytes(bytes) => {
                    request::Outcome::Success(Self(Either::Left(bytes)))
                }
                http_content_range::ContentRange::UnboundBytes(unbound) => {
                    request::Outcome::Success(Self(Either::Right(unbound)))
                }
                _ => request::Outcome::Error((Status::BadRequest, ())),
            },
            None => request::Outcome::Error((Status::BadRequest, ())),
        }
    }
}
