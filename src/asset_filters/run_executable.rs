use std::{
    collections::HashMap,
    fs::{create_dir, File},
    io::{copy, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use artushak_web_assets::{
    asset_filter::{get_string, get_string_list, option_is_flag, AssetFilter, AssetFilterOption},
    assets::{AssetError, AssetErrorType},
};
use log::debug;

use crate::asset_filters::AssetFilterCustomError;

pub struct AssetFilterRunExecutable {}

impl AssetFilter<AssetFilterCustomError> for AssetFilterRunExecutable {
    fn process_asset_file(
        &self,
        input_file_paths: &[PathBuf],
        output_file_path: &Path,
        options: &HashMap<String, AssetFilterOption>,
    ) -> Result<(), AssetError<AssetFilterCustomError>> {
        let executable_name = get_string(
            options
                .get("executable_name")
                .ok_or_else(|| {
                    AssetFilterCustomError::RequiredOptionMissing("executable_name".to_string())
                })?
                .clone(),
        )
        .ok_or_else(|| AssetFilterCustomError::InvalidOptionType("executable_name".to_string()))?;

        let input_file_argument_name = match options.get("input_file_argument_name") {
            Some(option) => Some(get_string(option.clone()).ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("input_file_argument_name".to_string())
            })?),
            None => None,
        };

        let output_file_argument_name = match options.get("output_file_argument_name") {
            Some(option) => Some(get_string(option.clone()).ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("output_file_argument_name".to_string())
            })?),
            None => None,
        };

        let pre_extra_arguments = match options.get("pre_extra_arguments") {
            Some(option) => Some(get_string_list(option.clone()).ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("pre_extra_arguments".to_string())
            })?),
            None => None,
        };

        let extra_arguments = match options.get("extra_arguments") {
            Some(option) => Some(get_string_list(option.clone()).ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("extra_arguments".to_string())
            })?),
            None => None,
        };

        let output_is_stdout = option_is_flag(options.get("output_is_stdout").cloned())
            .ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("output_is_stdout".to_string())
            })?;

        let input_is_stdin =
            option_is_flag(options.get("input_is_stdin").cloned()).ok_or_else(|| {
                AssetFilterCustomError::InvalidOptionType("input_is_stdin".to_string())
            })?;

        if input_file_paths.len() != 1 {
            // TODO: multiple input files
            return Err(AssetError::new(AssetErrorType::FilterError(
                AssetFilterCustomError::InvalidInputCount(input_file_paths.len()),
            )));
        }

        if let Some(output_file_path_parent) = output_file_path.parent() {
            if !output_file_path_parent.exists() {
                create_dir(output_file_path_parent).map_err(AssetFilterCustomError::IOError)?;
            }
        }

        let mut command = Command::new(executable_name);

        if let Some(pre_extra_arguments_real) = pre_extra_arguments {
            command.args(pre_extra_arguments_real);
        }

        if let Some(input_file_argument_name_real) = input_file_argument_name {
            command.arg(input_file_argument_name_real);
        }
        if !input_is_stdin {
            command.arg(input_file_paths[0].to_str().unwrap());
        } else {
            command.stdin(Stdio::piped());
        }

        if !output_is_stdout {
            if let Some(output_file_argument_name_real) = output_file_argument_name {
                command.arg(output_file_argument_name_real);
            }
            command.arg(output_file_path.to_str().unwrap());
        } else {
            command.stdout(Stdio::piped());
        }

        if let Some(extra_arguments_real) = extra_arguments {
            command.args(extra_arguments_real);
        }

        debug!("Running command {:#?}", command);

        let mut process = command.spawn()?;

        if input_is_stdin {
            let mut input_file = File::open(input_file_paths[0].to_str().unwrap())?;

            let mut stdin = process.stdin.take().unwrap();
            copy(&mut input_file, &mut stdin)?;
        }

        let output = process.wait_with_output()?;
        if !output.status.success() {
            return Err(AssetFilterCustomError::ExecutableStatusNotOk(output.status).into());
        }

        if output_is_stdout {
            let mut output_file = File::create(output_file_path)?;

            output_file.write_all(output.stdout.as_slice())?;
        }

        Ok(())
    }
}
