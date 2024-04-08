use std::{collections::HashMap, marker::PhantomData, pin::Pin, sync::Arc};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        channel::{self, Receiver, Sender},
        future::LazyFuture,
        protocol::{AsyncPacketRead, AsyncPacketSend},
        split::{ReadHalf, WriteHalf},
        task::{setter, Setter},
        token::IncToken,
        Stream,
    },
    error,
    runtime::Runtime,
};

use super::{lopper::Looper, AsyncCall, Cmd, Decoder};
use crate::core::rpc::Encoder;
use crate::core::split::SplitStream;

#[derive(Default, Clone)]
pub struct Calls {
    call_list: Arc<Mutex<HashMap<u64, Setter<Vec<u8>>>>>,
    inc_token: IncToken,
}

#[pin_project::pin_project]
pub struct Caller<S> {
    calls: Calls,
    request: Sender<Cmd>,
    #[pin]
    fut_call: Arc<Mutex<LazyFuture<'static, error::Result<Vec<u8>>>>>,
    marked: PhantomData<S>,
}

impl<'a, S> Caller<S>
where
    S: Stream + Send + Unpin + 'a,
{
    pub fn new<R>(stream: S, heartbeat_delay: std::time::Duration) -> (Looper<'a>, Self)
    where
        R: Runtime + 'a,
    {
        let (reader, writer) = stream.split();
        let (req_rx, req_ax) = channel::open::<Cmd>();
        let (heart_rx, heart_ax) = channel::open::<Response>();

        let calls = Calls::default();

        let mut looper = Looper::new();

        looper.post(Looper::run_heartbeat::<R>(
            heartbeat_delay,
            req_rx.clone(),
            heart_ax,
        ));

        looper.post(Looper::run_send_loop(calls.clone(), reader, heart_rx));

        looper.post(Looper::run_recv_loop(calls.clone(), writer, req_ax));

        (
            looper,
            Self {
                calls,
                request: req_rx,
                fut_call: Arc::new(Mutex::new(LazyFuture::new())),
                marked: PhantomData,
            },
        )
    }
}

impl<'caller, S, T> AsyncCall<T> for Caller<S>
where
    T: serde::Serialize + Send + 'static,
    S: Send + Unpin + 'caller,
{
    type Output = error::Result<Vec<u8>>;

    fn poll_call(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        arg: &T,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();

        let mut fut_call = this.fut_call.lock();

        Pin::new(&mut fut_call).poll(cx, move || {
            let data = arg.encode();
            let (setter, getter) = setter();
            let token = this.calls.add(setter);

            let result = match data {
                Ok(packet) => {
                    this.request.send_sync(Cmd::Transact { token, packet });
                    Ok(())
                }
                Err(error) => {
                    this.calls.cancel(token);
                    Err(error)
                }
            };

            async move {
                match result {
                    Ok(_) => getter.await,
                    Err(e) => Err(e),
                }
            }
        })
    }
}

impl<'a> Looper<'a> {
    async fn run_heartbeat<R>(
        delay: std::time::Duration,
        sender: Sender<Request>,
        receiver: Receiver<Response>,
    ) -> error::Result<()>
    where
        R: Runtime,
    {
        let mut last = std::time::Instant::now();

        loop {
            sender.send(Request::Ping).await?;

            match R::wait_for(delay, receiver.recv()).await?? {
                Response::Pong => {
                    last = std::time::Instant::now();
                }
                _ => unsafe { std::hint::unreachable_unchecked() },
            }

            R::sleep(delay).await;
        }
    }

    

    
}




impl<'a> std::future::Future for Looper<'a> {
    type Output = error::Result<()>;
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl Calls {
    fn add(&self, setter: Setter<Vec<u8>>) -> u64 {
        let mut calls = self.call_list.lock();

        let token = self.inc_token.next(|token| !calls.contains_key(&token));

        calls.insert(token, setter);

        token
    }

    fn wake(&self, token: u64, packet: Vec<u8>) -> error::Result<()> {
        match resp {
            Response::Pong => unsafe { std::hint::unreachable_unchecked() },
            Response::Data { token, data } => match self.call_list.lock().remove(&token) {
                None => Err(error::FusoError::BadRpcCall(token)),
                Some(setter) => setter.set(data),
            },
        }
    }

    fn cancel(&self, token: u64) {
        drop(self.call_list.lock().remove(&token))
    }

    fn cancel_all(&self) {
        self.call_list.lock().clear();
    }
}

impl<T> Clone for Caller<T> {
    fn clone(&self) -> Self {
        Caller {
            calls: self.calls.clone(),
            request: self.request.clone(),
            marked: PhantomData,
            fut_call: self.fut_call.clone(),
        }
    }
}
