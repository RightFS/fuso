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
    MsgPack(MsgPack),
    TomlDeError(toml::de::Error),
    StdIo(std::io::Error),
}

#[derive(Debug)]
pub enum MsgPack{
    Encode(rmp_serde::encode::Error),
    Decode(rmp_serde::decode::Error)
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

impl From<rmp_serde::encode::Error> for FusoError {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self::MsgPack(MsgPack::Encode(value))
    }
}

impl From<rmp_serde::decode::Error> for FusoError {
    fn from(value: rmp_serde::decode::Error) -> Self {
        Self::MsgPack(MsgPack::Decode(value))
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
