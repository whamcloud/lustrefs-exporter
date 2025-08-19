// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use sealed_test::prelude::tempfile::TempDir;
use std::{env, os::unix::fs::symlink, path::PathBuf};

const MOCK_CONFIG_DIR: &str = "MOCK_CONFIG_DIR";
const PATH: &str = "PATH";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to read mock file: {0}")]
    UnableToReadMockFile(String, String),
    #[error(transparent)]
    Var(#[from] std::env::VarError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A mock commander will set up the environment to mock external commands.
/// The `mock_command` method will return a builder that can be used to
/// define the arguments and output for the mock command.
pub struct MockCommander {
    mock_dir: TempDir,
    config_dir: TempDir,
}

impl Default for MockCommander {
    fn default() -> Self {
        let mock_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        // Using the temp directory, set the PATH environment variable to
        // include the mock_dir temporary directory at the beginning of the
        // PATH. This will ensure that any calls to the mocked command will
        // use the mock binary instead of the real binary.
        let current_path = env::var(PATH).unwrap_or_default();
        let new_path = format!("{}:{}", mock_dir.path().display(), current_path);

        unsafe {
            // Store the new path in the global PATH env variable.
            env::set_var(PATH, new_path);

            // Store the location of the temporary config_dir in a global
            // environment variable.
            env::set_var(MOCK_CONFIG_DIR, config_dir.path());
        }

        Self {
            mock_dir,
            config_dir,
        }
    }
}

impl MockCommander {
    pub fn mock_command(&self, command: &str) -> Result<MockBuilder, Error> {
        // Use a symlink to mock the specified command. This symlink will point to
        // the mock_bin binary.
        let mock_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("bin")
            .join("mock_bin");

        // Create a symlink from `mock_dir`/`command` -> `mock_bin`
        let link_path = self.mock_dir.path().join(command);

        if !link_path.exists() {
            symlink(mock_bin.clone(), link_path.clone())?;
        }

        Ok(MockBuilder {
            config_dir: self.config_dir.path().to_path_buf(),
            command: command.to_string(),
            args: None,
        })
    }
}

/// A Mock builder used to define the mock command's arguments and output.
pub struct MockBuilder {
    config_dir: PathBuf,
    command: String,
    args: Option<Vec<String>>,
}

impl MockBuilder {
    /// Define the arguments for the mock command.
    pub fn with_args(self, args: Vec<String>) -> Self {
        Self {
            args: Some(args),
            ..self
        }
    }

    /// Define the arguments for the mock command in a string format.
    pub fn with_args_string(self, args: String) -> Self {
        Self {
            args: Some(args.split(" ").map(String::from).collect::<Vec<_>>()),
            ..self
        }
    }

    /// Set the output for the mock command.
    pub fn set_output(self, output: String) -> Result<(), Error> {
        // Construct the filename. It should consist of:
        // <command_name>_<args joined by _>.response
        let filename = format!(
            "{}_{}.response",
            self.command,
            self.args.unwrap_or_default().join("_")
        );

        // It's possible that the filename will be too long, so hash the
        // filename using a deterministic algorithm to get a unique 16
        // character hex string.
        let hashed_filename = hash_filename(&filename);

        let response_file_path = self.config_dir.join(hashed_filename);

        std::fs::write(&response_file_path, output)?;

        Ok(())
    }

    /// Use the contents of the specified file as the output for the mock command.
    pub fn returns_file(self, file_path: &PathBuf) -> Result<(), Error> {
        // Read in the mock data to be returned
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            Error::UnableToReadMockFile(file_path.display().to_string(), e.to_string())
        })?;

        // Apply the mock data to a temporary file.
        self.set_output(content)
    }
}

/// Since filenames contain the args and this can potentially be very long, we
/// should create a hash from the filename to get a unique 16 character hex string.
fn hash_filename(filename: &str) -> String {
    let hash_filename = xxhash_rust::xxh3::xxh3_64(filename.as_bytes());

    format!("{:016x}", hash_filename)
}
