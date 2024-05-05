use std::{future::Future, sync::Arc};

use crate::error;

use super::BoxedFuture;

pub trait IntoConnector<'a, C, T, O> {
    fn into(self) -> AbstractConnector<'a, T, O>;
}

pub trait Connector<Target> {
    type Output;

    fn connect<'conn>(
        &'conn self,
        target: Target,
    ) -> BoxedFuture<'conn, error::Result<Self::Output>>;
}

pub struct AbstractConnector<'connector, T, O>(
    Box<dyn Connector<T, Output = O> + Send + Unpin + 'connector>,
);

pub struct ReplicableConnector<'connector, T, O>(pub Arc<AbstractConnector<'connector, T, O>>);

pub struct ConnectorWithFn<'connector, T, O> {
    f: Box<dyn Fn(T) -> BoxedFuture<'connector, error::Result<O>> + Send + Sync>,
}

pub struct MultiConnector<'a, T, O> {
    connectors: Vec<AbstractConnector<'a, T, O>>,
}

unsafe impl<T, O> Sync for AbstractConnector<'_, T, O> {}

impl<'connector, T, O> AbstractConnector<'connector, T, O> {
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

    pub fn add<I, C>(&mut self, connector: I)
    where
        I: IntoConnector<'a, C, T, O>,
    {
        self.connectors.push(connector.into());
    }
}

impl<T, O> Connector<T> for AbstractConnector<'_, T, O> {
    type Output = O;
    fn connect<'conn>(&'conn self, target: T) -> BoxedFuture<'conn, error::Result<Self::Output>> {
        self.0.connect(target)
    }
}

impl<'connector, T, O> Connector<T> for ReplicableConnector<'connector, T, O> {
    type Output = O;
    fn connect<'conn>(&'conn self, target: T) -> BoxedFuture<'conn, error::Result<Self::Output>> {
        self.0.connect(target)
    }
}

impl<T, O> Connector<T> for MultiConnector<'_, T, O>
where
    T: Clone + Send,
{
    type Output = O;

    fn connect<'conn>(&'conn self, target: T) -> BoxedFuture<'conn, error::Result<Self::Output>> {
        Box::pin(async move {
            for connector in self.connectors.iter() {
                if let Ok(o) = connector.connect(target.clone()).await {
                    return Ok(o);
                }
            }

            Err(error::FusoError::InvalidConnection)
        })
    }
}

impl<'a, C, T, O> IntoConnector<'a, C, T, O> for C
where
    C: Connector<T, Output = O> + Send + Unpin + 'a,
{
    fn into(self) -> AbstractConnector<'a, T, O> {
        AbstractConnector::new(self)
    }
}

impl<'a, F, Fut, T, O> IntoConnector<'a, ConnectorWithFn<'a, T, O>, T, O> for F
where
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = error::Result<O>> + Send + 'a,
    T: 'a,
    O: 'static,
{
    fn into(self) -> AbstractConnector<'a, T, O> {
        AbstractConnector::new(ConnectorWithFn {
            f: Box::new(move |t| Box::pin((self)(t))),
        })
    }
}

impl<'a, T, O> Connector<T> for ConnectorWithFn<'a, T, O> {
    type Output = O;
    fn connect<'conn>(&'conn self, target: T) -> BoxedFuture<'conn, error::Result<Self::Output>> {
        (self.f)(target)
    }
}

impl<'connector, T, O> Clone for ReplicableConnector<'connector, T, O> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
