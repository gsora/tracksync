use crate::*;

#[derive(Debug)]
pub enum Error {
    ValidationError(String),
    DatabaseError(sqlx::Error),
    IOError(std::io::Error),
    CopyError(fs_extra::error::Error),
    MediaFileError(audiotags::Error),
    FilterError(filter::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ValidationError(ve) => write!(f, "validation error: {:?}", ve),
            Error::DatabaseError(de) => match de {
                sqlx::Error::Database(de) => write!(f, "database error: {}", de.message()),
                sqlx::Error::Protocol(pe) => write!(f, "database error: {pe}"),
                other => write!(f, "database error: {:?}", other),
            },
            Error::IOError(io) => write!(f, "IO error: {:?}", io),
            Error::CopyError(ce) => write!(f, "file copy error kind: {:?}", ce.kind),
            Error::MediaFileError(mfe) => write!(f, "media file error error: {:?}", mfe),
            Error::FilterError(fe) => write!(f, "Filtering error: {:?}", fe),
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Self::DatabaseError(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<fs_extra::error::Error> for Error {
    fn from(value: fs_extra::error::Error) -> Self {
        Self::CopyError(value)
    }
}

impl From<audiotags::Error> for Error {
    fn from(value: audiotags::Error) -> Self {
        Self::MediaFileError(value)
    }
}

impl From<filter::Error> for Error {
    fn from(value: filter::Error) -> Self {
        Self::FilterError(value)
    }
}
impl std::error::Error for Error {}
