use std::fmt::Display;

use crate::{
    core::{
        connector::{Connector, ReplicableConnector},
        io::AsyncWriteExt,
        rpc::{
            structs::port_forward::{Request, Response},
            Responder,
        },
        transfer::{AbstractTransmitter, TransmitterExt},
        Connection,
    },
    error,
};

pub struct Linker {
    token: u64,
    connector: ReplicableConnector<'static, Protocol, AbstractTransmitter<'static>>,
    responder: Responder,
}

pub enum Reason {
    Cancel,
}

#[derive(Debug, Clone)]
pub enum Protocol {
    Tcp,
    Udp,
    Kcp,
}

impl Linker {
    pub fn new(
        token: u64,
        connector: ReplicableConnector<'static, Protocol, AbstractTransmitter<'static>>,
        responder: Responder,
    ) -> Self {
        Self {
            token,
            connector,
            responder,
        }
    }

    pub async fn link(self, poto: Protocol) -> error::Result<AbstractTransmitter<'static>> {
        self.responder.replay(Response::Ok).await?;

        let mut transmitter = self.connector.connect(poto).await?;
        let token = self.token.to_be_bytes();

        transmitter.send_all(&token).await?;

        Ok(transmitter)
    }

    pub async fn cancel<R>(self, reason: R) -> error::Result<()>
    where
        R: Into<Reason>,
    {
        let reason = reason.into();

        Ok(())
    }
}

impl From<()> for Reason {
    fn from(value: ()) -> Self {
        Self::Cancel
    }
}

impl Display for Linker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "token={}", self.token)
    }
}
