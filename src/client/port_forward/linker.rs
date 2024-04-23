use std::fmt::Display;

use crate::{
    core::{rpc::Responder, Connection},
    error,
};

pub struct Linker {
    token: u64,
    responder: Responder,
}

impl Linker {
    pub fn new(token: u64, responder: Responder) -> Self {
        Self { token, responder }
    }

    pub async fn link(self) -> error::Result<()> {
        unimplemented!()
    }

    pub async fn reject(self) -> error::Result<()> {
        unimplemented!()
    }
}


impl Display for Linker{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "token={}", self.token)
    }
}