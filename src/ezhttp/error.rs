use std::error::Error;

/// Http error
#[derive(Debug)]
pub enum HttpError {
    ReadLineEof,
    ReadLineUnknown,
    InvalidHeaders,
    InvalidQuery,
    InvalidContentSize,
    InvalidContent,
    JsonParseError,
    WriteHeadError,
    WriteBodyError,
    InvalidStatus,
    RequstError
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for HttpError {}
