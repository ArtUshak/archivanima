use std::{
    collections::HashMap,
    env::current_dir,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use artushak_web_assets::{
    asset_filter::{AssetFilter, AssetFilterOption},
    assets::{AssetError, AssetErrorType},
};
use figment::util::diff_paths;
use log::debug;
use tempfile::tempdir;

use crate::asset_filters::AssetFilterCustomError;

pub struct AssetFilterTsc {
    pub tsc_name: Option<String>,
    pub args: Vec<String>,
}

impl AssetFilter<AssetFilterCustomError> for AssetFilterTsc {
    fn process_asset_file(
        &self,
        input_file_paths: &[PathBuf],
        output_file_path: &Path,
        _options: &HashMap<String, AssetFilterOption>,
    ) -> Result<(), AssetError<AssetFilterCustomError>> {
        let mut command = Command::new(self.tsc_name.as_deref().unwrap_or("tsc"));

        if input_file_paths.len() != 1 {
            return Err(AssetError::new(AssetErrorType::FilterError(
                AssetFilterCustomError::InvalidInputCount(input_file_paths.len()),
            )));
        }

        let input_file_path = &input_file_paths[0];

        command.arg(input_file_path);
        command.args(self.args.clone());

        let temp_directory = tempdir()?;
        command.arg("--outDir");
        command.arg(temp_directory.path());
        command.arg("--rootDir");
        command.arg(".");

        debug!("Running command {:#?}", command);

        let mut process = command.spawn()?;

        let status = process.wait()?;
        if !status.success() {
            return Err(AssetFilterCustomError::ExecutableStatusNotOk(status).into());
        }

        let processed_input_file_path = diff_paths(input_file_path, current_dir()?).ok_or(
            AssetError::new(AssetErrorType::FilterError(
                AssetFilterCustomError::InvalidPath(input_file_path.clone()),
            )),
        )?;

        let temp_file_path = temp_directory
            .path()
            .join(processed_input_file_path.with_extension("js"));

        debug!(
            "Copying temporary file {} to output {}",
            temp_file_path.display(),
            output_file_path.display()
        );
        fs::copy(temp_file_path, output_file_path)?;

        Ok(())
    }
}
