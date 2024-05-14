// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use combine::error::StringStreamError;
use std::{io, str};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LustreCollectorError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error(transparent)]
    SerdeYamlError(#[from] serde_yaml::Error),
    #[error(transparent)]
    StringStreamError(#[from] StringStreamError),
    #[error(transparent)]
    CombineEasyError(combine::stream::easy::Errors<char, &'static str, usize>),
    #[error(transparent)]
    Utf8Error(#[from] str::Utf8Error),
    #[error("{0}")]
    ConversionError(String),
    #[error("Cannot convert timestamp {0} to a u64 of milliseconds")]
    InvalidTime(String),
}

impl From<combine::stream::easy::Errors<char, &str, usize>> for LustreCollectorError {
    fn from(err: combine::stream::easy::Errors<char, &str, usize>) -> Self {
        LustreCollectorError::CombineEasyError(err.map_range(|_| ""))
    }
}
