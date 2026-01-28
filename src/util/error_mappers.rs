use minijinja::Error;
use serde::de::Error as _;

pub struct IOError {
    e: std::io::Error,
}

impl From<std::io::Error> for IOError {
    fn from(e: std::io::Error) -> Self {
        IOError { e }
    }
}
impl Into<Error> for IOError {
    fn into(self) -> Error {
        Error::custom(format!("IO Error: {}", self.e)).with_source(self.e)
    }
}

pub struct TomlError {
    e: toml::de::Error,
}

impl From<toml::de::Error> for TomlError {
    fn from(e: toml::de::Error) -> Self {
        TomlError { e }
    }
}
impl Into<Error> for TomlError {
    fn into(self) -> Error {
        Error::custom(format!("Toml Error: {}", self.e)).with_source(self.e)
    }
}
pub fn map_toml_error(e: toml::de::Error) -> Error {
    TomlError::from(e).into()
}


pub fn map_io_error(e: std::io::Error) -> Error {
    IOError::from(e).into()
}
