use std::marker::PhantomData;

use crate::core::{channel, rpc::Looper};

use super::AsyncCallee;

pub struct Callee<O> {
    _marked: PhantomData<O>,
}

impl<S> Callee<S> {
    pub fn new(stream: S, heartbeat_delay: std::time::Duration) -> Self {
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
