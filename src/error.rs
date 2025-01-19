use std::error::Error;

/// Http library errors
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
    RequestError,
    UrlError,
    ConnectError,
    ShutdownError,
    SslError,
    UnknownScheme
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for HttpError {}
