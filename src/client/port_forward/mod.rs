mod linker;
mod transport;

use self::linker::Linker;
use crate::core::rpc::{structs::port_forward::Request, AsyncCallee};
use std::{marker::PhantomData, pin::Pin, task::Poll};

use crate::{
    client::port_forward::transport::Transport,
    config::client::FinalTarget,
    core::{accepter::Accepter, connector::Connector, Connection, Stream},
    runtime::Runtime,
};

pub struct PortForwarder<R, S> {
    transport: Transport<S>,
    _marked: PhantomData<R>,
}

impl<R, S> PortForwarder<R, S>
where
    R: Runtime + 'static,
    S: Stream + Unpin + Send + 'static,
{
    pub fn new_with_runtime<C>(transport: S, connector: C) -> Self
    where
        C: Connector<(), Output = Connection<'static>>,
    {
        // let (transport, hold) = Transport::new::<R>(std::time::Duration::from_secs(1), transport);

        unimplemented!()
    }
}

impl<R, S> Accepter for PortForwarder<R, S>
where
    S: Unpin,
{
    type Output = (Linker, FinalTarget);

    fn poll_accept(
        mut self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<Self::Output>> {
        // match Pin::new(&mut self.transport).poll_next(ctx) {
        //     std::task::Poll::Pending => Poll::Pending,
        //     std::task::Poll::Ready(Request::New(addr, a)) => {
        //         unimplemented!()
        //     },
        // };

        unimplemented!()
    }
}
