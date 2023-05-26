use std::{io, path::PathBuf, process::ExitStatus};

use artushak_web_assets::assets::AssetFilterError;

pub mod run_executable;
pub mod tsc;

#[derive(Debug)]
pub enum AssetFilterCustomError {
    IOError(io::Error),
    InvalidInputCount(usize),
    RequiredOptionMissing(String),
    InvalidOptionType(String),
    ExecutableStatusNotOk(ExitStatus),
    InvalidPath(PathBuf),
}

impl AssetFilterError for AssetFilterCustomError {}

impl From<io::Error> for AssetFilterCustomError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}
