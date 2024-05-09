use std::{marker::PhantomData, task::Poll};

use fuso_socks::Socks;

use crate::{
    config::client::{Addr, ServerAddr},
    core::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{AsyncRecvExt, AsyncRecvFromExt, AsyncSendToExt, UdpProvider},
        processor::Preprocessor,
        rpc::structs::port_forward::{Target, VisitorProtocol},
        stream::virio::{self, Vitio},
        transfer::TransmitterExt,
        AbstractStream,
    },
    error,
};

use super::Connection;

macro_rules! unpack_socks_addr {
    ($addr: expr) => {
        match $addr {
            fuso_socks::Addr::Socket(ip, port) => (ServerAddr(vec![Addr::WithIpAddr(ip)]), port),
            fuso_socks::Addr::Domain(domain, port) => {
                (ServerAddr(vec![Addr::WithDomain(domain)]), port)
            }
        }
    };
}

pub struct UdpForwarder {}

pub struct Socks5Preprocessor<P> {
    udp_forwarder: UdpForwarder,
    _marked: PhantomData<P>,
}

impl Preprocessor<Connection> for () {
    type Output = error::Result<VisitorProtocol>;

    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        Box::pin(async move { Ok(VisitorProtocol::Other(input, None)) })
    }
}

impl Preprocessor<Connection> for Option<()> {
    type Output = error::Result<Connection>;
    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        Box::pin(async move { Ok(input) })
    }
}

impl futures::AsyncRead for Connection {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        crate::core::io::AsyncRead::poll_read(self, cx, buf).map_err(Into::into)
    }
}

impl futures::AsyncWrite for Connection {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        crate::core::io::AsyncWrite::poll_write(self, cx, buf).map_err(Into::into)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        crate::core::io::AsyncWrite::poll_flush(self, cx).map_err(Into::into)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl<P> Socks5Preprocessor<P> {
    pub fn new() -> Self {
        Socks5Preprocessor {
            udp_forwarder: UdpForwarder {},
            _marked: PhantomData,
        }
    }
}

impl<P> Preprocessor<Connection> for Socks5Preprocessor<P>
where
    P: UdpProvider + Sync + 'static,
{
    type Output = error::Result<VisitorProtocol>;
    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        Box::pin(async move {
            let mut input = input;

            input.mark();

            Ok({
                match fuso_socks::Socks::parse(&mut input, None).await? {
                    Socks::Invalid => {
                        input.reset();
                        VisitorProtocol::Other(input, None)
                    }

                    Socks::Tcp(addr) => {
                        input.discard();
                        let (addr, port) = unpack_socks_addr!(addr);
                        VisitorProtocol::Socks(input, Target::Tcp(addr, port))
                    }
                    Socks::Udp(addr) => {
                        input.discard();
                        let (addr, port) = unpack_socks_addr!(addr);

                        let conn = self.udp_forwarder.open(input).await?;

                        VisitorProtocol::Socks(conn, Target::Udp(addr, port))
                    }
                }
            })
        })
    }
}



impl UdpForwarder {
    pub async fn open(&self, conn: Connection) -> error::Result<Connection> {
        let (vio1, vio2) = virio::open();

        let new_conn = Connection::new(conn.addr().clone(), AbstractStream::new(vio1));

        Ok(new_conn)
    }
}


async fn enter_udp_forward<P: UdpProvider>(
    mut vio: Vitio,
    mut input: Connection,
) -> error::Result<()> {
    let (listen, udp) = crate::core::net::UdpSocket::bind::<P, _>(([0, 0, 0, 0], 0)).await?;

    let mut buf = Vec::new();

    buf.extend(&[0x05, 0x00, 0x00]);

    match listen {
        std::net::SocketAddr::V4(addr) => {
            buf.push(0x01);
            buf.extend(addr.ip().octets());
            buf.extend(addr.port().to_be_bytes());
        }
        std::net::SocketAddr::V6(_) => todo!(),
    }

    input.write_all(&buf).await?;

    let mut udp = udp;
    let mut buf = [0u8; 1024];
    let (addr, n) = udp.recvfrom(&mut buf).await.unwrap();

    vio.send_all(&buf[..n]).await;

    let a = vio.read(&mut buf).await?;

    Ok(())
}
