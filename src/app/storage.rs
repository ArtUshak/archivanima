use std::io::SeekFrom;

use log::debug;
use tokio::{
    fs::{File, OpenOptions},
    io::{copy, AsyncRead, AsyncSeekExt},
};

use crate::{utils::try_remove_file, UploadStorage};

pub fn get_file_name(id: i64, extension: Option<&str>) -> String {
    match extension {
        Some(extension) => format!("{:016x}.{}", id, extension),
        None => format!("{:016x}", id),
    }
}

pub fn get_file_url<'a, 'b: 'a>(
    id: i64,
    extension: Option<&'a str>,
    storage: &'b UploadStorage,
) -> String {
    match storage {
        UploadStorage::FileSystem {
            private_path: _,
            public_path: _,
            base_url,
        } => {
            format!("{}{}", base_url, get_file_name(id, extension))
        }
    }
}

pub async fn allocate_private_file(
    id: i64,
    extension: Option<&str>,
    size: u64,
    storage: &UploadStorage,
) -> std::io::Result<()> {
    match storage {
        UploadStorage::FileSystem {
            private_path,
            public_path: _,
            base_url: _,
        } => {
            let file_path = private_path.join(get_file_name(id, extension));
            debug!("Allocating file {}", file_path.display());
            let mut file = File::create(file_path).await?;
            file.seek(SeekFrom::Start(size)).await?;
            Ok(())
        }
    }
}

pub async fn write_private_file<'r, 'a, R>(
    id: i64,
    extension: Option<&str>,
    data: &mut R,
    start_pos: u64,
    storage: &UploadStorage,
) -> std::io::Result<()>
where
    R: AsyncRead,
    R: Unpin,
{
    match storage {
        UploadStorage::FileSystem {
            private_path,
            public_path: _,
            base_url: _,
        } => {
            let file_path = private_path.join(get_file_name(id, extension));
            let mut file = OpenOptions::new().write(true).open(file_path).await?;
            file.seek(SeekFrom::Start(start_pos)).await?;
            copy(data, &mut file).await?;
            Ok(())
        }
    }
}

pub async fn publish_file<'r, 'a>(
    id: i64,
    extension: Option<&str>,
    storage: &UploadStorage,
) -> std::io::Result<()> {
    match storage {
        UploadStorage::FileSystem {
            private_path,
            public_path,
            base_url: _,
        } => {
            let file_name = get_file_name(id, extension);
            tokio::fs::copy(private_path.join(&file_name), public_path.join(file_name)).await?;
            // TODO: symlink
            Ok(())
        }
    }
}

pub async fn unpublish_file<'r, 'a>(
    id: i64,
    extension: Option<&str>,
    storage: &UploadStorage,
) -> std::io::Result<()> {
    match storage {
        UploadStorage::FileSystem {
            private_path,
            public_path,
            base_url: _,
        } => {
            let file_name = get_file_name(id, extension);
            try_remove_file(public_path.join(&file_name)).await?;
            try_remove_file(private_path.join(file_name)).await?;
            Ok(())
        }
    }
}
