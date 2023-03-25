use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use artushak_web_assets::{
    asset_filter::{AssetFilter, AssetFilterOption},
    assets::{AssetError, AssetErrorType},
};
use rsass::compile_scss;

use crate::asset_filters::AssetFilterCustomError;

pub struct AssetFilterSCSS {
    pub format: rsass::output::Format,
}

impl AssetFilter<AssetFilterCustomError> for AssetFilterSCSS {
    fn process_asset_file(
        &self,
        input_file_paths: &[PathBuf],
        output_file_path: &Path,
        _options: &HashMap<String, AssetFilterOption>,
    ) -> Result<(), AssetError<AssetFilterCustomError>> {
        if input_file_paths.len() != 1 {
            return Err(AssetError::new(AssetErrorType::FilterError(
                AssetFilterCustomError::InvalidInputCount(input_file_paths.len()),
            )));
        }

        let input_file_content = std::fs::read(&input_file_paths[0])?;

        let output_file_content = compile_scss(input_file_content.as_slice(), self.format)
            .map_err(AssetFilterCustomError::from)?;

        if let Some(output_file_path_parent) = output_file_path.parent() {
            std::fs::create_dir_all(output_file_path_parent)?;
        }
        std::fs::write(output_file_path, output_file_content)?;

        Ok(())
    }
}
