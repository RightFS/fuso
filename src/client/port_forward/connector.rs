use rc4::{KeyInit, Rc4, StreamCipher};

use crate::core::{
    connector::{AbstractConnector, Connector, IntoConnector},
    stream::handshake::MuxConfig,
    transfer::{AbstractTransmitter, TransmitterExt},
};

use super::Protocol;

pub struct MuxConnector<'connector> {
    conf: MuxConfig,
    inner: AbstractConnector<'connector, Protocol, AbstractTransmitter<'connector>>,
}

impl<'connector> MuxConnector<'connector> {
    pub fn new<I, C>(conf: MuxConfig, connector: I) -> Self
    where
        I: IntoConnector<'connector, C, Protocol, AbstractTransmitter<'connector>>,
    {
        Self {
            conf,
            inner: connector.into(),
        }
    }
}

impl<'connector> Connector<Protocol> for MuxConnector<'connector> {
    type Output = AbstractTransmitter<'connector>;

    fn connect<'conn>(
        &'conn self,
        target: Protocol,
    ) -> crate::core::BoxedFuture<'conn, crate::error::Result<Self::Output>> {
        Box::pin(async move {
            let mut transmitter = self.inner.connect(target).await?;
            let mut magic = self.conf.magic.to_le_bytes();

            Rc4::new((&self.conf.secret).into()).apply_keystream(&mut magic);

            transmitter.send_all(&magic).await?;

            Ok(transmitter)
        })
    }
}
