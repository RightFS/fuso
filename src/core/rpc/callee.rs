use std::marker::PhantomData;

use crate::{
    core::{
        channel::{self, Receiver, Sender},
        protocol::AsyncPacketRead,
        rpc::{lopper::Looper, Looper},
        split::{ReadHalf, WriteHalf},
        Stream,
    },
    error,
};

use super::{structs::port_forward::Request, AsyncCallee, Decoder};
use crate::core::split::SplitStream;

pub struct Callee<O> {
    _marked: PhantomData<O>,
}

impl<S> Callee<S>
where
    S: Stream + Send + Unpin,
{
    pub fn new(stream: S, heartbeat_delay: std::time::Duration) -> Self {
        let (looper, rx, ax) = Looper::new(stream);

        unimplemented!()
    }
}



impl<O> AsyncCallee for Callee<O> {
    type Output = O;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unimplemented!()
    }
}

impl Looper<'_> {
    async fn run_callee_receiver<S>(
        reader: ReadHalf<S>,
        sender: Sender<Request>,
    ) -> error::Result<()>
    where
        S: Stream + Unpin,
    {
        let mut reader = reader;
        loop {
            let pkt = reader.recv_packet().await?;
            sender.send(pkt.decode()?).await?;
        }
    }

    async fn run_callee_sender<S>(
        writer: WriteHalf<S>,
        receiver: Receiver<Request>,
    ) -> error::Result<()>
    where
        S: Stream + Unpin,
    {
        loop {
            let data = receiver.recv().await?;
        }
    }
}
