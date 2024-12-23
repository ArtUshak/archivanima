use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use bcrypt::{hash, verify};
use rand::{distributions::Standard, Rng};
use rocket::{
    async_trait,
    fairing::{self, Fairing as RocketFairing, Info, Kind},
    http::{Cookie, Status},
    request::{FromRequest, Outcome},
    time::{Duration, OffsetDateTime},
    Data, Request, Rocket, State,
};
use std::borrow::Cow;

// taken from https://github.com/kotovalexarian/rocket_csrf

const BCRYPT_COST: u32 = 8;

#[derive(Debug, Clone)]
pub struct CsrfConfig {
    /// CSRF Cookie lifespan
    lifespan: Duration,
    /// CSRF cookie name
    cookie_name: Cow<'static, str>,
    /// CSRF Token character length
    cookie_len: usize,
}

pub struct Fairing {
    config: CsrfConfig,
}

pub struct CsrfToken(pub String);

pub struct VerificationFailure;

impl Default for Fairing {
    fn default() -> Self {
        Self::new(CsrfConfig::default())
    }
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            lifespan: Duration::days(1), // TODO: refresh
            cookie_name: "csrf_token".into(),
            cookie_len: 32,
        }
    }
}

impl Fairing {
    pub fn new(config: CsrfConfig) -> Self {
        Self { config }
    }
}

#[allow(dead_code)]
impl CsrfConfig {
    /// Set CSRF lifetime (expiration time) for cookie.
    ///
    pub fn with_lifetime(mut self, time: Duration) -> Self {
        self.lifespan = time;
        self
    }

    /// Set CSRF Cookie Name.
    ///
    pub fn with_cookie_name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.cookie_name = name.into();
        self
    }

    /// Set CSRF Cookie length, keep this above or equal to 16 in size.
    ///
    pub fn with_cookie_len(mut self, length: usize) -> Self {
        self.cookie_len = length;
        self
    }
}

impl CsrfToken {
    pub fn authenticity_token(&self) -> String {
        hash(&self.0, BCRYPT_COST).unwrap()
    }

    pub fn verify(&self, form_authenticity_token: &str) -> Result<(), VerificationFailure> {
        if verify(&self.0, form_authenticity_token).unwrap_or(false) {
            Ok(())
        } else {
            Err(VerificationFailure {})
        }
    }
}

#[async_trait]
impl RocketFairing for Fairing {
    fn info(&self) -> Info {
        Info {
            name: "CSRF",
            kind: Kind::Ignite | Kind::Request,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<rocket::Build>) -> fairing::Result {
        Ok(rocket.manage(self.config.clone()))
    }

    async fn on_request(&self, request: &mut Request<'_>, _: &mut Data<'_>) {
        let config = request.guard::<&State<CsrfConfig>>().await.unwrap();

        if request.valid_csrf_token_from_session(config).is_some() {
            return;
        }

        let values: Vec<u8> = rand::thread_rng()
            .sample_iter(Standard)
            .take(config.cookie_len)
            .collect();

        let encoded = BASE64_STANDARD_NO_PAD.encode(&values[..]);

        let expires = OffsetDateTime::now_utc() + config.lifespan;

        request
            .cookies()
            .add_private(Cookie::build((config.cookie_name.clone(), encoded)).expires(expires));
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for CsrfToken {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config = request.guard::<&State<CsrfConfig>>().await.unwrap();

        match request.valid_csrf_token_from_session(config) {
            None => Outcome::Error((Status::Forbidden, ())),
            Some(token) => Outcome::Success(Self(BASE64_STANDARD_NO_PAD.encode(token))),
        }
    }
}

trait RequestCsrf {
    fn valid_csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>> {
        self.csrf_token_from_session(config).and_then(|raw| {
            if raw.len() >= config.cookie_len {
                Some(raw)
            } else {
                None
            }
        })
    }

    fn csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>>;
}

impl RequestCsrf for Request<'_> {
    fn csrf_token_from_session(&self, config: &CsrfConfig) -> Option<Vec<u8>> {
        let cookies = self.cookies();
        cookies
            .get_private(&config.cookie_name)
            .or_else(|| cookies.get_pending(&config.cookie_name))
            .and_then(|cookie| BASE64_STANDARD_NO_PAD.decode(cookie.value()).ok())
    }
}
