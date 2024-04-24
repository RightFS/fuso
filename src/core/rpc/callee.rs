use std::{future::Future, marker::PhantomData, pin::Pin, task::Poll};

use crate::{
    core::{
        channel::{self, Receiver, Sender},
        future::{Poller, Select},
        protocol::AsyncPacketRead,
        rpc::polling::Looper,
        split::{ReadHalf, WriteHalf},
        task::{setter, Getter, Setter},
        Stream,
    },
    error,
};

use super::{structs::port_forward::Request, AsyncCallee, Cmd, Decoder, Transact};
use crate::core::split::SplitStream;

pub struct Callee<'looper> {
    guarder: Sender<(u64, Getter<()>)>,
    requester: Receiver<Transact>,
    responder: Sender<Cmd>,
    _marked: PhantomData<&'looper ()>,
}

pub struct Responder {
    token: u64,
    guard: Setter<()>,
    responder: Sender<Cmd>,
}

impl<'looper> Callee<'looper> {
    pub fn new<S>(stream: S, heartbeat_delay: std::time::Duration) -> (Looper<'looper>, Self)
    where
        S: Stream + Send + Unpin + 'looper,
    {
        let (mut looper, rx, ax) = Looper::new(stream);

        let (sender, receiver) = channel::open();

        looper.post(Responder::run_responder_guarder(receiver, rx.clone()));

        (
            looper,
            Self {
                guarder: sender,
                requester: ax,
                responder: rx,
                _marked: PhantomData,
            },
        )
    }
}

impl Responder {
    async fn run_responder_guarder(
        rx: Receiver<(u64, Getter<()>)>,
        sender: Sender<Cmd>,
    ) -> error::Result<()> {
        let mut calls = Vec::new();
        std::future::poll_fn(|cx| {
            let mut polled = false;

            while !polled {
                polled = match rx.poll_recv(cx)? {
                    Poll::Pending => true,
                    Poll::Ready((token, getter)) => {
                        calls.push((token, getter));
                        false
                    }
                };

                calls.retain_mut(|(token, getter)| match Pin::new(getter).poll(cx) {
                    Poll::Pending => true,
                    Poll::Ready(Ok(())) => false,
                    Poll::Ready(Err(_)) => {
                        sender.send_sync(Cmd::Cancel(*token));
                        false
                    }
                });
            }

            Poll::Pending
        })
        .await
    }
}


impl AsyncCallee for Callee<'_> {
    type Output = error::Result<(Vec<u8>, Responder)>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        loop {
            break match self.requester.poll_recv(cx)? {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Transact::Cancel(token)) => {
                    log::debug!("cancel {token}");
                    // TODO
                    continue;
                }
                Poll::Ready(Transact::Request(token, packet)) => Poll::Ready(Ok({
                    let (setter, getter) = setter();

                    self.guarder.send_sync((token, getter));

                    (
                        packet,
                        Responder {
                            token,
                            guard: setter,
                            responder: self.responder.clone(),
                        },
                    )
                })),
            };
        }
    }
}


impl Drop for Responder {
    fn drop(&mut self) {
        self.guard.invalid();
        log::debug!("cleaned callee {}", self.token);
    }
}
