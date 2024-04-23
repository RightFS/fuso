use std::{marker::PhantomData, task::Poll};

use crate::{
    core::{
        channel::{self, Receiver, Sender},
        protocol::AsyncPacketRead,
        rpc::lopper::Looper,
        split::{ReadHalf, WriteHalf},
        Stream,
    },
    error,
};

use super::{structs::port_forward::Request, AsyncCallee, Cmd, Decoder};
use crate::core::split::SplitStream;

pub struct Callee<'looper> {
    requester: Receiver<(u64, Vec<u8>)>,
    responder: Sender<Cmd>,
    _marked: PhantomData<&'looper ()>,
}

pub struct Responder {
    token: u64,
    responder: Sender<Cmd>,
}

impl<'looper> Callee<'looper> {
    pub fn new<S>(stream: S, heartbeat_delay: std::time::Duration) -> (Looper<'looper>, Self)
    where
        S: Stream + Send + Unpin + 'looper,
    {
        let (looper, rx, ax) = Looper::new(stream);

        (
            looper,
            Self {
                requester: ax,
                responder: rx,
                _marked: PhantomData,
            },
        )
    }
}

impl AsyncCallee for Callee<'_> {
    type Output = error::Result<(Vec<u8>, Responder)>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        match self.requester.poll_recv(cx)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready((token, packet)) => Poll::Ready(Ok((
                packet,
                Responder {
                    token,
                    responder: self.responder.clone(),
                },
            ))),
        }
    }
}
