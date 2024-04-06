use std::marker::PhantomData;

use crate::core::{
    rpc::{structs::port_forward::Request, AsyncCallee},
    Stream,
};

pub struct Transport<S> {
  _marked: PhantomData<S>
}

impl<S> Transport<S> {
    pub fn new<R>(heartbeat_delay: std::time::Duration, stream: S) -> Self
    where
        S: Stream + Send + Unpin,
    {
        unimplemented!()
    }
}


impl<S> AsyncCallee for Transport<S> {
    type Output = Request;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unimplemented!()
    }
}
