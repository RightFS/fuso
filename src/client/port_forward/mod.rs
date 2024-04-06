mod linker;

use std::marker::PhantomData;
use self::linker::Linker;

use crate::{
    config::client::FinalTarget,
    core::{
        accepter::Accepter,
        port_forward::Transport,
        rpc::{structs::port_forward::Request, AsyncCall},
        Connection, Stream,
    },
    runtime::Runtime,
};



pub struct PortForwarder<R> {
    _marked: PhantomData<R>,
}

impl<R> PortForwarder<R>
where
    R: Runtime + 'static,
{
    pub fn new_with_runtime<S, C>(transport: S, connector: C) -> Self
    where
        S: Stream + Unpin + Send + 'static,
    {
        let (transport, hold) = Transport::new::<R>(std::time::Duration::from_secs(1), transport);

        unimplemented!()
    }
}



impl<R> Accepter for PortForwarder<R> {
    type Output = (Linker, FinalTarget);

    fn poll_accept(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<Self::Output>> {
        todo!()
    }
}
