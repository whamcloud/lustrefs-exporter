// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{env, path::PathBuf};

const MOCK_CONFIG_DIR: &str = "MOCK_CONFIG_DIR";

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Command name not found")]
    CommandNameNotFound,
    #[error("Config directory not found")]
    ConfigDirNotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn main() -> Result<(), Error> {
    // The first argument will be the path to the symlinked path. Extract the filename
    // from that path to determine which command is being called.
    let command_name = env::args()
        .next()
        .and_then(|path| {
            PathBuf::from(path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .ok_or(Error::CommandNameNotFound)?;

    // Record all arguments
    let args = env::args().skip(1).collect::<Vec<_>>();

    // Get the config directory associated with the test
    let config_dir = env::var(MOCK_CONFIG_DIR).map_err(|_| Error::ConfigDirNotFound)?;

    // Construct the path to the mock response file
    let mock_filename = format!("{command_name}_{}.response", args.join("_"));
    let hashed_filename = hash_filename(&mock_filename);

    let mock_match = PathBuf::from(config_dir).join(hashed_filename.clone());

    if PathBuf::from(&mock_match).exists() {
        let Ok(content) = std::fs::read_to_string(&mock_match) else {
            return Err(Error::Io(std::io::Error::other(format!(
                "Unable to read mock response file (`{}`).",
                mock_match.display()
            ))));
        };

        print!("{content}");
    } else {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Mock response file (`{}`) not found", mock_match.display()),
        )));
    }

    Ok(())
}

/// Since filenames contain the args and this can potentially be very long, we
/// should create a hash from the filename to get a unique 16 character hex string.
fn hash_filename(filename: &str) -> String {
    let hash_filename = xxhash_rust::xxh3::xxh3_64(filename.as_bytes());

    format!("{:016x}", hash_filename)
}
