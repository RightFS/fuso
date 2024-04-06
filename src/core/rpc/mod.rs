mod callee;
mod caller;

pub mod structs;

use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use crate::error;

pub use callee::*;
pub use caller::*;

use super::future::Select;

#[pin_project::pin_project]
pub struct Call<'caller, C, A, O> {
    arg: A,
    #[pin]
    caller: &'caller mut C,
    _marked: PhantomData<O>,
}

pub trait AsyncCall<T> {
    type Output;

    fn poll_call(self: Pin<&mut Self>, cx: &mut Context<'_>, arg: &T) -> Poll<Self::Output>;
}

pub trait AsyncCallee {
    type Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub trait ICallExt<A, O>: AsyncCall<A, Output = O> {
    fn call<'caller>(&'caller mut self, arg: A) -> Call<'caller, Self, A, O>
    where
        Self: Sized + Unpin,
    {
        Call {
            caller: self,
            arg,
            _marked: PhantomData,
        }
    }
}

pub trait Encoder<T> {
    fn encode(self) -> error::Result<Vec<u8>>;
}

pub trait Decoder<T> {
    type Output;
    fn decode(self) -> error::Result<Self::Output>;
}

impl<T, A, O> ICallExt<A, O> for T where T: AsyncCall<A, Output = O> {}

pub struct Looper<'looper>(Select<'looper, error::Result<()>>);

impl<T> Encoder<T> for T
where
    T: serde::Serialize,
{
    fn encode(self) -> error::Result<Vec<u8>> {
        bincode::serialize(&self).map_err(Into::into)
    }
}

impl<T> Decoder<T> for Vec<u8>
where
    T: serde::de::DeserializeOwned,
{
    type Output = T;

    fn decode(self) -> error::Result<Self::Output> {
        bincode::deserialize(&self).map_err(Into::into)
    }
}

impl<'caller, C, A, O> std::future::Future for Call<'caller, C, A, O>
where
    C: AsyncCall<A, Output = O> + Unpin,
{
    type Output = O;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        Pin::new(&mut **this.caller).poll_call(cx, &this.arg)
    }
}

impl<'looper> Looper<'looper> {
    fn new() -> Self {
        Self(Select::new())
    }

    fn post<F>(&mut self, f: F)
    where
        F: Future<Output = error::Result<()>> + Send + 'looper,
    {
        self.0.add(f);
    }
}
