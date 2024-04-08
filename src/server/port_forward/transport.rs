use std::pin::Pin;
use std::task::Poll;

use crate::core::rpc::{self, Decoder};
use crate::core::rpc::{Caller, Looper};

use crate::runtime::Runtime;
use crate::{
    core::{
        rpc::{
            structs::port_forward::{Request, Response},
            AsyncCall,
        },
        Stream,
    },
    error,
};

pub struct Transport<T> {
    caller: Caller<T>,
}

pub struct TransportHold(Looper<'static>);

impl<'a, T> Transport<T>
where
    T: Stream + Send + Unpin + 'static,
{
    pub fn new<R>(heartbeat_delay: std::time::Duration, stream: T) -> (Self, TransportHold)
    where
        R: Runtime + 'static,
    {
        let (looper, caller) = rpc::Caller::new::<R>(stream, heartbeat_delay);

        (Self { caller }, TransportHold(looper))
    }
}

impl<T> AsyncCall<Request> for Transport<T>
where
    T: Stream + Send + Unpin,
{
    type Output = error::Result<Response>;

    fn poll_call(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        arg: &Request,
    ) -> std::task::Poll<Self::Output> {
        match Pin::new(&mut self.caller).poll_call(cx, arg)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(data) => Poll::Ready(Ok(data.decode()?)),
        }
    }
}

impl<T> Clone for Transport<T> {
    fn clone(&self) -> Self {
        Self {
            caller: self.caller.clone(),
        }
    }
}

impl std::future::Future for TransportHold {
    type Output = error::Result<()>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}