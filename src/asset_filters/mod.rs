use std::{io, process::ExitStatus};

use artushak_web_assets::assets::AssetFilterError;

pub mod run_executable;
pub mod scss2css;

#[derive(Debug)]
pub enum AssetFilterCustomError {
    IOError(io::Error),
    InvalidInputCount(usize),
    RequiredOptionMissing(String),
    InvalidOptionType(String),
    RSASSError(Box<rsass::Error>),
    ExecutableStatusNotOk(ExitStatus),
}

impl AssetFilterError for AssetFilterCustomError {}

impl From<io::Error> for AssetFilterCustomError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

impl From<rsass::Error> for AssetFilterCustomError {
    fn from(err: rsass::Error) -> Self {
        Self::RSASSError(Box::new(err))
    }
}
