use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::error;

pub trait TransSender {
    fn poll_send(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<error::Result<usize>>;
}

pub trait TransReceiver {
    fn poll_recv(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<error::Result<usize>>;
}


