mod connector;
mod linker;
mod transport;

pub use linker::*;
pub use connector::*;


use crate::core::connector::ReplicableConnector;
use crate::core::transfer::AbstractTransmitter;
use crate::{
    core::{
        connector::AbstractConnector,
        rpc::{structs::port_forward::Request, AsyncCallee},
    },
    error,
};
use std::sync::Arc;
use std::{marker::PhantomData, pin::Pin, task::Poll};

use crate::{
    client::port_forward::transport::Transport,
    config::client::FinalTarget,
    core::{accepter::Accepter, connector::Connector, Connection, Stream},
    runtime::Runtime,
};

pub struct PortForwarder<R, S> {
    transport: Transport<'static, S>,
    connector: ReplicableConnector<'static, Protocol, AbstractTransmitter<'static>>,
    _marked: PhantomData<R>,
}

impl<R, S> PortForwarder<R, S>
where
    R: Runtime + 'static,
    S: Stream + Unpin + Send + 'static,
{
    pub fn new_with_runtime<C>(transport: S, connector: C) -> Self
    where
        C: Connector<Protocol, Output = AbstractTransmitter<'static>> + Sync + Send + Unpin + 'static,
    {
        let transport = Transport::new::<R>(std::time::Duration::from_secs(1), transport);

        Self {
            transport,
            connector: ReplicableConnector(Arc::new(AbstractConnector::new(connector))),
            _marked: PhantomData,
        }
    }
}

impl<R, S> Accepter for PortForwarder<R, S>
where
    R: Unpin,
    S: Stream + Send + Unpin,
{
    type Output = (Linker, FinalTarget);

    fn poll_accept(
        mut self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<Self::Output>> {
        match Pin::new(&mut self.transport).poll_next(ctx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready((request, responder)) => match request {
                Request::New(token, target) => Poll::Ready(Ok({
                    let linker = Linker::new(token, self.connector.clone(), responder);
                    (linker, {
                        match target {
                            None => FinalTarget::Dynamic,
                            Some(target) => FinalTarget::Dynamic,
                        }
                    })
                })),
            },
        }
    }
}
