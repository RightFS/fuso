mod accepter;
mod handshake;
mod preprocessor;
mod transport;

pub use accepter::*;
pub use handshake::*;
pub use preprocessor::*;

use parking_lot::Mutex;
use std::future::Future;

use std::marker::PhantomData;
use std::{collections::HashMap, pin::Pin, sync::Arc, task::Poll};

use crate::core::future::Poller;
use crate::core::io::AsyncReadExt;
use crate::core::rpc::structs::port_forward::{self};
use crate::core::rpc::AsyncCall;
use crate::core::token::IncToken;
use crate::core::Stream;
use crate::runtime::Runtime;
use crate::{
    core::{
        accepter::Accepter,
        io::{AsyncRead, AsyncWrite},
        processor::{Preprocessor, WrappedPreprocessor},
        rpc::structs::port_forward::{Request, VisitorProtocol},
    },
    error,
};

use transport::Transport;

type Connection = crate::core::Connection<'static>;

enum Outcome {
    Ready(u64, Connection),
    Pending(u64),
    Timeout(u64),
    Transport(u64, error::FusoError),
    Stopped(Option<error::FusoError>),
}

pub enum Whence {
    Visitor(Connection),
    Mapping(Connection),
}

#[derive(Clone)]
pub struct Visitors {
    inc_token: IncToken,
    connections: Arc<Mutex<HashMap<u64, Connection>>>,
}

pub struct PortForwarder<Runtime, A, T> {
    poller: Poller<'static, error::Result<Outcome>>,
    accepter: A,
    visitors: Visitors,
    transport: Transport<T>,
    prepmap: WrappedPreprocessor<'static, Connection, error::Result<Connection>>,
    prepvis: WrappedPreprocessor<'static, Connection, error::Result<VisitorProtocol>>,
    _marked: PhantomData<Runtime>,
}

impl<R, A, T> Accepter for PortForwarder<R, A, T>
where
    A: Accepter<Output = Whence> + Unpin + 'static,
    T: Stream + Unpin + Send + 'static,
    R: Runtime + Unpin + 'static,
{
    type Output = (Connection, Connection);

    fn poll_accept(
        mut self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<Self::Output>> {
        let mut polled = false;

        while !polled {
            let whence = match Pin::new(&mut self.accepter).poll_accept(ctx)? {
                std::task::Poll::Pending => None,
                std::task::Poll::Ready(whence) => Some(whence),
            };

            polled = match whence {
                None => true,
                Some(Whence::Visitor(conn)) => {
                    let fut = Self::do_prepare_visitor(
                        conn,
                        self.visitors.clone(),
                        self.transport.clone(),
                        self.prepvis.clone(),
                    );

                    self.poller.add(fut);

                    false
                }
                Some(Whence::Mapping(conn)) => {
                    let fut = Self::do_prepare_mapping(
                        conn,
                        self.transport.clone(),
                        self.prepmap.clone(),
                    );

                    self.poller.add(fut);

                    false
                }
            };

            let (need_poll, outcome) = match Pin::new(&mut self.poller).poll(ctx) {
                Poll::Pending => continue,
                Poll::Ready(Ok(o)) => (false, o),
                Poll::Ready(Err(e)) => {
                    log::error!("{:?}", e);
                    continue;
                }
            };

            polled = need_poll;

            match outcome {
                Outcome::Pending(token) => {
                    self.poller.add(Self::wait_transport(
                        token,
                        std::time::Duration::from_secs(10),
                    ));
                }
                Outcome::Timeout(token) => {
                    if let Some(conn) = self.visitors.take(token) {
                        log::warn!(
                            "failed to create mapping {{ token={}, addr={}, msg='timeout' }}",
                            token,
                            conn.addr()
                        );
                    }
                }
                Outcome::Ready(token, transport) => {
                    match self.visitors.take(token) {
                        Some(visitor) => return Poll::Ready(Ok((visitor, transport))),
                        None => log::warn!("invalid mapping because the client has been closed {{ token={token} }}")
                    };
                }
                Outcome::Stopped(error) => {
                    return {
                        match error {
                            Some(e) => Poll::Ready(Err(e)),
                            None => Poll::Ready(Err(error::FusoError::Abort)),
                        }
                    }
                }
                Outcome::Transport(token, transport) => {
                    if let Some(conn) = self.visitors.take(token) {
                        log::warn!(
                            "failed to create mapping {{ token={}, addr={}, msg='{}' }}",
                            token,
                            conn.addr(),
                            transport
                        );
                    }
                }
            };
        }

        Poll::Pending
    }
}

#[cfg(feature = "fuso-runtime")]
impl<R, A, T> PortForwarder<R, A, T>
where
    A: Accepter<Output = Whence> + Unpin + 'static,
    T: Stream + Unpin + Send + 'static,
    R: Runtime + 'static,
{
    pub fn new_with_runtime<P, M>(stream: T, accepter: A, prepvis: P, prepmap: M) -> Self
    where
        P: Preprocessor<Connection, Output = error::Result<VisitorProtocol>>,
        P: Send + Sync + 'static,
        M: Preprocessor<Connection, Output = error::Result<Connection>>,
        M: Send + Sync + 'static,
    {
        let mut poller = Poller::new();

        let (transport, hold) = Transport::new::<R>(std::time::Duration::from_secs(10), stream);

        poller.add(async move {
            match hold.await {
                Ok(()) => Ok(Outcome::Stopped(None)),
                Err(e) => Ok(Outcome::Stopped(Some(e))),
            }
        });

        Self {
            poller,
            accepter,
            transport,
            visitors: Default::default(),
            prepvis: WrappedPreprocessor(Arc::new(prepvis)),
            prepmap: WrappedPreprocessor(Arc::new(prepmap)),
            _marked: PhantomData,
        }
    }
}

impl<R, A, T> PortForwarder<R, A, T>
where
    R: Runtime,
    T: AsyncWrite + AsyncRead + Send + Unpin,
{
    async fn wait_transport(token: u64, timeout: std::time::Duration) -> error::Result<Outcome> {
        log::debug!("wait for mapping {{ token={token}, timeout={timeout:?} }}");
        R::sleep(timeout).await;
        Ok(Outcome::Timeout(token))
    }

    async fn do_prepare_visitor(
        conn: Connection,
        visitors: Visitors,
        transport: Transport<T>,
        preprocessor: WrappedPreprocessor<'_, Connection, error::Result<VisitorProtocol>>,
    ) -> error::Result<Outcome> {
        let mut transport = transport;

        let (conn, addr) = match preprocessor.prepare(conn).await? {
            VisitorProtocol::Socks(conn, socks) => (conn, Some(socks)),
            VisitorProtocol::Other(conn, addr) => match addr {
                None => (conn, None),
                Some(target) => (conn, Some(target)),
            },
        };

        log::debug!("create mapping {} -- [T] -- {:?}", conn.addr(), addr);

        let token = visitors.store(conn);

        let result = R::wait_for(std::time::Duration::from_secs(10), async move {
            match transport.call(Request::New(token, addr)).await {
                Err(e) => Err(e),
                Ok(resp) => match resp {
                    port_forward::Response::Ok => Ok(()),
                    port_forward::Response::Error(msg) => Err(msg.into()),
                },
            }
        })
        .await;

        match result {
            Err(_) => Ok(Outcome::Transport(token, error::FusoError::NotResponse)),
            Ok(r) => match r {
                Ok(_) => Ok(Outcome::Pending(token)),
                Err(e) => Ok(Outcome::Transport(token, e)),
            },
        }
    }

    async fn do_prepare_mapping(
        conn: Connection,
        _: Transport<T>,
        preprocessor: WrappedPreprocessor<'_, Connection, error::Result<Connection>>,
    ) -> error::Result<Outcome> {
        let mut conn = preprocessor.prepare(conn).await?;

        let mut buf = [0u8; 8];

        conn.read_exact(&mut buf).await?;

        let token = u64::from_be_bytes(buf);

        log::debug!("created mapping {{ token={token} }}");

        Ok(Outcome::Ready(token, conn))
    }
}

impl Default for Visitors {
    fn default() -> Self {
        Self {
            inc_token: Default::default(),
            connections: Default::default(),
        }
    }
}

impl Visitors {
    fn take(&self, token: u64) -> Option<Connection> {
        self.connections.lock().remove(&token)
    }

    fn store(&self, conn: Connection) -> u64 {
        let token = self.next_token();

        self.connections.lock().insert(token, conn);

        token
    }

    fn next_token(&self) -> u64 {
        let connections = self.connections.lock();
        self.inc_token
            .next(|token| !connections.contains_key(&token))
    }
}
