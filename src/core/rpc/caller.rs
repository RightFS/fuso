use std::{collections::HashMap, marker::PhantomData, pin::Pin, sync::Arc};

use parking_lot::Mutex;

use crate::{
    core::{
        channel::{Receiver, Sender},
        future::LazyFuture,
        task::{setter, Setter},
        token::IncToken,
        Stream,
    },
    error,
    runtime::Runtime,
};

use super::{lopper::Looper, AsyncCall, Cmd};
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
    caller: Arc<Mutex<LazyFuture<'static, error::Result<Vec<u8>>>>>,
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
        let calls = Calls::default();

        let (mut looper, sender, receiver) = Looper::new(stream);

        looper.post(Looper::run_command_consumer::<R>(receiver, calls.clone()));

        (
            looper,
            Self {
                calls,
                request: sender,
                caller: Arc::new(Mutex::new(LazyFuture::new())),
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

        let mut caller = this.caller.lock();

        Pin::new(&mut caller).poll(cx, move || {
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
    async fn run_command_consumer<R>(
        receiver: Receiver<(u64, Vec<u8>)>,
        calls: Calls,
    ) -> error::Result<()>
    where
        R: Runtime,
    {
        loop {
            let (token, packet) = receiver.recv().await?;
            calls.wake(token, packet)?;
        }
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
        match self.call_list.lock().remove(&token) {
            None => Err(error::FusoError::BadRpcCall(token)),
            Some(setter) => setter.set(packet),
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
            caller: self.caller.clone(),
        }
    }
}
