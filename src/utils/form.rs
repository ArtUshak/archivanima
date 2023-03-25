use crate::utils::csrf_lib::{CsrfToken, VerificationFailure};

pub trait CheckCSRF {
    fn check_csrf(&self, token: &CsrfToken) -> Result<(), VerificationFailure>;
}
