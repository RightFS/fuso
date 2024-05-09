use std::{
    collections::VecDeque,
    io::Read,
    pin::Pin,
    sync::Arc,
    task::{Poll, Waker},
};

use parking_lot::Mutex;

use crate::{
    core::io::{AsyncRead, AsyncWrite},
    error,
};

type VirWaker = Arc<Mutex<Option<Waker>>>;

#[derive(Clone, Debug)]
struct Container {
    total: usize,
    maxbuf: usize,
    buffer: VecDeque<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct VirBuf {
    container: Arc<Mutex<Container>>,
    read_waker: VirWaker,
    write_waker: VirWaker,
}

#[derive(Clone, Debug)]
pub struct Vitio {
    left: VirBuf,
    right: VirBuf,
}

impl AsyncRead for Vitio {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<crate::error::Result<usize>> {
        Pin::new(&mut self.left).poll_read(cx, buf)
    }
}

impl AsyncWrite for Vitio {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<crate::error::Result<usize>> {
        Pin::new(&mut self.right).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl VirBuf {
    fn new(read_waker: VirWaker, write_waker: VirWaker) -> Self {
        Self {
            read_waker,
            write_waker,
            container: Default::default(),
        }
    }
}

impl Default for Container {
    fn default() -> Self {
        Self {
            total: 0,
            maxbuf: 5,
            buffer: Default::default(),
        }
    }
}

impl AsyncRead for VirBuf {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<crate::error::Result<usize>> {
        let mut ct = self.container.lock();
        if ct.total == 0 {
            self.read_waker.lock().replace(cx.waker().clone());
            Poll::Pending
        } else {
            let n = ct.fill(buf)?;
            ct.total -= n;
            drop(ct);
            self.try_wake_write();
            Poll::Ready(Ok(n))
        }
    }
}

impl AsyncWrite for VirBuf {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<crate::error::Result<usize>> {
        if !self.can_fill() {
            self.write_waker.lock().replace(cx.waker().clone());
            Poll::Pending
        } else {
            {
                let mut ct = self.container.lock();
                ct.buffer.push_back(buf.to_vec());
                ct.total += buf.len();
            }

            self.try_wake_read();
            Poll::Ready(Ok(buf.len()))
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::error::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl VirBuf {
    fn can_fill(&self) -> bool {
        let ct = self.container.lock();
        ct.buffer.len() < ct.maxbuf
    }

    fn try_wake_write(&mut self) {
        if let Some(waker) = self.write_waker.lock().take() {
            waker.wake();
        }
    }

    fn try_wake_read(&mut self) {
        if let Some(waker) = self.read_waker.lock().take() {
            waker.wake();
        }
    }
}

impl Container {
    fn fill(&mut self, buf: &mut [u8]) -> error::Result<usize> {
        let mut off = 0;

        while off < buf.len() {
            match self.buffer.pop_front() {
                None => break,
                Some(mut data) => {
                    let total = data.len();

                    let mut vio = std::io::Cursor::new(&mut data);

                    let n = vio.read(&mut buf[off..])?;

                    off += n;

                    if n < total {
                        self.buffer.push_front(buf[n..].to_vec());
                    }
                }
            }
        }

        Ok(off)
    }
}

pub fn open() -> (Vitio, Vitio) {
    let read_waker = VirWaker::default();
    let write_waker = VirWaker::default();

    let left = VirBuf::new(read_waker, write_waker);

    let read_waker = VirWaker::default();
    let write_waker = VirWaker::default();

    let right = VirBuf::new(read_waker, write_waker);

    let v1 = Vitio {
        left: right.clone(),
        right: left.clone(),
    };

    let v2 = Vitio { left, right };

    (v1, v2)
}

#[cfg(test)]
mod tests {
    use crate::core::io::{AsyncReadExt, AsyncWriteExt};

    use super::open;

    #[tokio::test]
    async fn k() {
        let (mut v1, mut v2) = open();

        tokio::spawn(async move {
            loop {
                println!("call1 ...");

                let mut buf = [0u8; 1024];

                let n = v2.read(&mut buf).await.unwrap();

                println!("{:?}", &buf[..n]);

                v2.write_all(&mut buf[..n]).await.unwrap();
            }
        });

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        println!("call2 ...");

        v1.write(&[0x1, 0x2]).await.unwrap();
        v1.write(&[0x1, 0x2]).await.unwrap();

        let mut buf = [0u8; 1024];

        let n = v1.read(&mut buf).await.unwrap();

        println!("{:?}", &buf[..n]);

        v1.write(&[0x1, 0x2]).await.unwrap();

        let n = v1.read(&mut buf).await.unwrap();

        println!("{:?}", &buf[..n])
    }
}
