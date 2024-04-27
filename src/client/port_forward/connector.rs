use std::marker::PhantomData;

use crate::core::{
    connector::{AbstractConnector, Connector, IntoConnector},
    io::AsyncWriteExt,
    protocol::AsyncPacketSend,
    transfer::AbstractTransmitter,
};

use super::Protocol;

pub struct MuxConnector<'connector> {
    inner: AbstractConnector<'connector, Protocol, AbstractTransmitter<'connector>>,
}

impl<'connector> MuxConnector<'connector> {
    pub fn new<I, C>(connector: I) -> Self
    where
        I: IntoConnector<'connector, C, Protocol, AbstractTransmitter<'connector>>,
    {
        Self {
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
            let transmitter = self.inner.connect(target).await?;

            unimplemented!()
        })
    }
}
