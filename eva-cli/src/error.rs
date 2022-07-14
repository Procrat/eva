use failure::Fail;

use crate::{configuration, parse};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Configuration(#[cause] configuration::Error),
    #[fail(display = "{}", _0)]
    Parse(#[cause] parse::Error),
    #[fail(display = "{}", _0)]
    Eva(#[cause] eva::Error),
}

impl From<configuration::Error> for Error {
    fn from(error: configuration::Error) -> Error {
        Error::Configuration(error)
    }
}

impl From<parse::Error> for Error {
    fn from(error: parse::Error) -> Error {
        Error::Parse(error)
    }
}

impl From<eva::Error> for Error {
    fn from(error: eva::Error) -> Error {
        Error::Eva(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
