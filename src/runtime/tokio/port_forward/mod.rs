use std::net::SocketAddr;

use crate::{
    core::{
        accepter::Accepter, connector::Connector, processor::Preprocessor,
        rpc::structs::port_forward::VisitorProtocol, BoxedStream, Connection, Stream,
    },
    error,
    server::port_forward::{MuxAccepter, Whence},
};

use super::TokioRuntime;

impl<A> MuxAccepter<TokioRuntime, A>
where
    A: Accepter<Output = (SocketAddr, BoxedStream<'static>)> + Unpin + Send,
{
    pub fn new(magic: u32, secret: [u8; 16], accepter: A) -> Self {
        Self::new_runtime(accepter, magic, secret)
    }
}

impl<A, T> crate::server::port_forward::PortForwarder<TokioRuntime, A, T>
where
    A: Accepter<Output = Whence> + Unpin + 'static,
    T: Stream + Send + Unpin + 'static,
{
    pub fn new<P, M>(stream: T, accepter: A, prepvis: P, prepmap: M) -> Self
    where
        P: Preprocessor<Connection<'static>, Output = error::Result<VisitorProtocol>>,
        P: Send + Sync + 'static,
        M: Preprocessor<Connection<'static>, Output = error::Result<Connection<'static>>>,
        M: Send + Sync + 'static,
    {
        Self::new_with_runtime(stream, accepter, prepvis, prepmap)
    }
}

impl<S> crate::client::port_forward::PortForwarder<TokioRuntime, S>
where
    S: Stream + Send + Unpin + 'static,
{
    pub fn new<C>(transport: S, connector: C) -> Self
    where
        C: Connector<(), Output = Connection<'static>> + Send + Unpin + 'static,
    {
        Self::new_with_runtime(transport, connector)
    }
}
