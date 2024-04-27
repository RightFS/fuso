use std::fmt::Display;

pub type Result<T> = std::result::Result<T, FusoError>;

#[derive(Debug)]
pub enum FusoError {
    BadMagic,
    Timeout,
    Abort,
    AuthError,
    Cancel,
    InvalidConnection,
    InvaledSetter,
    BadRpcCall(u64),
    InvalidPort,
    InvalidExposeType,
    NotResponse,
    Custom(String),
    Bincode(bincode::Error),
    TomlDeError(toml::de::Error),
    StdIo(std::io::Error),
}

impl From<std::io::Error> for FusoError {
    fn from(value: std::io::Error) -> Self {
        Self::StdIo(value)
    }
}

impl From<toml::de::Error> for FusoError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDeError(value)
    }
}

impl From<bincode::Error> for FusoError {
    fn from(value: bincode::Error) -> Self {
        Self::Bincode(value)
    }
}

impl From<String> for FusoError {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

impl Display for FusoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
