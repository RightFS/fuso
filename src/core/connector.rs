use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::error;

pub trait Connector<Target> {
    type Output;

    fn poll_connect(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        target: &Target,
    ) -> Poll<error::Result<Self::Output>>;
}

#[pin_project::pin_project]
pub struct Connect<'a, C, T> {
    target: T,
    #[pin]
    connector: &'a mut C,
}

pub trait ConnectExt<T, O>: Connector<T, Output = O> {
    fn connect<'a>(&'a mut self, target: T) -> Connect<'a, Self, T>
    where
        Self: Sized,
    {
        Connect {
            target,
            connector: self,
        }
    }
}

impl<C, T, O> ConnectExt<T, O> for C where C: Connector<T, Output = O> {}

pub struct BoxedConnector<'connector, T, O>(
    Box<dyn Connector<T, Output = O> + Send + Unpin + 'connector>,
);

pub struct MultiConnector<'a, T, O> {
    connectors: Vec<BoxedConnector<'a, T, O>>,
}

impl<'connector, T, O> BoxedConnector<'connector, T, O> {
    pub fn new<C>(connector: C) -> Self
    where
        C: Connector<T, Output = O> + Send + Unpin + 'connector,
    {
        Self(Box::new(connector))
    }
}

impl<'a, T, O> MultiConnector<'a, T, O> {
    pub fn new() -> Self {
        Self {
            connectors: Default::default(),
        }
    }

    pub fn add<C>(&mut self, connector: C)
    where
        C: Connector<T, Output = O> + Send + Unpin + 'a,
    {
        self.connectors.push(BoxedConnector(Box::new(connector)));
    }
}

impl<C, T, O> std::future::Future for Connect<'_, C, T>
where
    C: Connector<T, Output = O> + Unpin,
{
    type Output = error::Result<C::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        Pin::new(&mut **this.connector).poll_connect(cx, &this.target)
    }
}

impl<T, O> Connector<T> for BoxedConnector<'_, T, O> {
    type Output = O;
    fn poll_connect(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        target: &T,
    ) -> Poll<error::Result<Self::Output>> {
        Pin::new(&mut *self.0).poll_connect(cx, target)
    }
}

impl<T, O> Connector<T> for MultiConnector<'_, T, O> {
    type Output = O;

    fn poll_connect(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        target: &T,
    ) -> Poll<error::Result<Self::Output>> {
        let mut error = Option::None;

        for connector in self.connectors.iter_mut() {
            match Pin::new(connector).poll_connect(cx, target) {
                Poll::Pending => continue,
                Poll::Ready(Ok(o)) => return Poll::Ready(Ok(o)),
                Poll::Ready(Err(e)) => {
                    error.replace(e);
                }
            }
        }

        match error {
            None => Poll::Pending,
            Some(e) => Poll::Ready(Err(e)),
        }
    }
}
