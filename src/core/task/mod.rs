use std::{
    sync::Arc,
    task::{Poll, Waker},
};

use parking_lot::Mutex;

use crate::error;

mod pool;

pub enum ValState<V> {
    Ok(V),
    Nil,
    Invalid,
}

pub struct Getter<V> {
    val: Arc<Mutex<ValState<V>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

pub struct Setter<V> {
    val: Arc<Mutex<ValState<V>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<V> Setter<V> {
    pub fn set(mut self, val: V) -> error::Result<()> {
        drop(self.val.lock().replace(val));
        self.try_wake();
        Ok(())
    }

    pub fn invalid(&mut self) {
        self.val.lock().invalid();
        self.try_wake();
    }

    fn try_wake(&mut self) {
        if let Some(waker) = self.waker.lock().take() {
            waker.wake();
        };
    }
}

impl<V> Drop for Setter<V> {
    fn drop(&mut self) {
        self.invalid();
    }
}

impl<V> std::future::Future for Getter<V> {
    type Output = error::Result<V>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut val = self.val.lock();
        match val.take()? {
            Some(v) => Poll::Ready(Ok(v)),
            None => {
                self.waker.lock().replace(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl<V> ValState<V> {
    fn take(&mut self) -> error::Result<Option<V>> {
        match std::mem::replace(self, Self::Nil) {
            ValState::Nil => Ok(None),
            ValState::Ok(v) => Ok(Some(v)),
            ValState::Invalid => Err(error::FusoError::InvaledSetter),
        }
    }

    fn replace(&mut self, val: V) -> Self {
        std::mem::replace(self, Self::Ok(val))
    }

    fn invalid(&mut self) {
        drop(std::mem::replace(self, Self::Invalid));
    }
}

pub fn setter<V>() -> (Setter<V>, Getter<V>) {
    let val = Arc::new(Mutex::new(ValState::Nil));

    (
        Setter {
            val: val.clone(),
            waker: Default::default(),
        },
        Getter {
            val,
            waker: Default::default(),
        },
    )
}
