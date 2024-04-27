use std::{
    io,
    pin::{self, Pin},
    task::{Context, Poll},
};

use crate::error;

use super::Stream;

#[pin_project::pin_project]
pub struct TransmitSend<'t, T> {
    data: &'t [u8],
    #[pin]
    transmitter: &'t mut T,
}

#[pin_project::pin_project]
pub struct TransmitSendAll<'t, T> {
    pos: usize,
    data: &'t [u8],
    #[pin]
    transmitter: &'t mut T,
}

pub trait Transmitter: Unpin {
    fn poll_send(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<error::Result<usize>>;

    fn poll_recv(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<error::Result<usize>>;
}

pub trait TransmitterExt: Transmitter {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransmitSend<'a, Self>
    where
        Self: Sized,
    {
        TransmitSend {
            data,
            transmitter: self,
        }
    }

    fn send_all<'a>(&'a mut self, data: &'a [u8]) -> TransmitSendAll<'a, Self>
    where
        Self: Sized,
    {
        TransmitSendAll {
            pos: 0,
            data,
            transmitter: self,
        }
    }
}

impl<T> TransmitterExt for T where T: Transmitter {}

pub struct AbstractTransmitter<'transmitter> {
    inner: Box<dyn Transmitter + Send + Unpin + 'transmitter>,
}

impl<'transmitter> Transmitter for AbstractTransmitter<'transmitter> {
    fn poll_send(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<error::Result<usize>> {
        Pin::new(&mut *self.inner).poll_send(ctx, buf)
    }

    fn poll_recv(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<error::Result<usize>> {
        Pin::new(&mut *self.inner).poll_recv(ctx, buf)
    }
}

impl<T> Transmitter for T
where
    T: Stream + Unpin,
{
    fn poll_send(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<error::Result<usize>> {
        T::poll_write(self, ctx, buf)
    }

    fn poll_recv(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<error::Result<usize>> {
        T::poll_read(self, ctx, buf)
    }
}

impl<'transmitter, S> From<S> for AbstractTransmitter<'transmitter>
where
    S: Stream + Send + Unpin + 'transmitter,
{
    fn from(value: S) -> Self {
        Self {
            inner: Box::new(value),
        }
    }
}

impl<'t, T> std::future::Future for TransmitSend<'t, T>
where
    T: Transmitter,
{
    type Output = error::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        Pin::new(&mut **this.transmitter).poll_send(cx, &this.data)
    }
}

impl<'t, T> std::future::Future for TransmitSendAll<'t, T>
where
    T: Transmitter,
{
    type Output = error::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        while *this.pos < this.data.len() {
            match Pin::new(&mut **this.transmitter).poll_send(cx, &this.data[*this.pos..])? {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(0) => {
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe).into()))
                }
                Poll::Ready(n) => {
                    *this.pos += n;
                }
            };
        }

        Poll::Ready(Ok(()))
    }
}
