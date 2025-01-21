use std::fmt;

use uniffi::Error;

pub type Result<T, E = LniSdkError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
#[uniffi(flat_error)]
pub enum LniSdkError {
    Generic(String),
}

impl fmt::Display for LniSdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic(e) => write!(f, "{e}"),
        }
    }
}

impl<T> From<T> for LniSdkError
where
    T: std::error::Error,
{
    fn from(e: T) -> LniSdkError {
        Self::Generic(e.to_string())
    }
}
