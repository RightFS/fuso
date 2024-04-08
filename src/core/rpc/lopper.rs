use std::future::Future;

use crate::{
    core::{
        channel::{self, Receiver, Sender},
        future::Select,
        protocol::{AsyncPacketRead, AsyncPacketSend},
        split::{ReadHalf, WriteHalf},
        Stream,
    },
    error,
};

use super::{keep::Heartbeat, Cmd, Decoder, Encoder};
use crate::core::split::SplitStream;

pub struct Looper<'looper>(Select<'looper, error::Result<()>>);

impl<'looper> Looper<'looper> {
    pub(super) fn new<S>(stream: S) -> (Self, Sender<Cmd>, Receiver<(u64, Vec<u8>)>)
    where
        S: Stream + Unpin + Send + 'looper,
    {
        let (prx, pax) = channel::open();
        let (psrx, psax) = channel::open();

        let (poller, heartbeat) = Heartbeat::new(prx.clone());

        let (reader, writer) = stream.split();

        let mut select = Select::new();

        select.add(poller);
        select.add(Self::run_recv_loop(writer, pax));
        select.add(Self::run_send_loop(reader, psrx, heartbeat));

        (Self(select), prx, psax)
    }

    pub(super) fn post<F>(&mut self, f: F)
    where
        F: Future<Output = error::Result<()>> + Send + 'looper,
    {
        self.0.add(f);
    }
}

impl<'looper> Looper<'looper> {
    pub(super) async fn run_send_loop<S>(
        reader: ReadHalf<S>,
        sender: Sender<(u64, Vec<u8>)>,
        heartbeat: Heartbeat,
    ) -> error::Result<()>
    where
        S: Stream + Unpin,
    {
        let mut reader = reader;

        loop {
            let data = reader.recv_packet().await?;
            let cmd: Cmd = data.decode()?;
            match cmd {
                Cmd::Pong => heartbeat.pong(),
                Cmd::Ping => heartbeat.ping(),
                Cmd::Transact { token, packet } => {
                    heartbeat.interrupt();
                    sender.send((token, packet)).await?;
                }
            };
        }
    }

    pub(super) async fn run_recv_loop<S>(
        writer: WriteHalf<S>,
        receiver: Receiver<Cmd>,
    ) -> error::Result<()>
    where
        S: Stream + Unpin,
    {
        let mut writer = writer;
        loop {
            let pkt = receiver.recv().await?.encode()?;
            writer.send_packet(&pkt).await?;
        }
    }
}
