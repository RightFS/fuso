use std::task::Poll;

use tokio::io::ReadBuf;

use crate::core::io::{AsyncRead, AsyncWrite};

impl AsyncRead for tokio::io::Stdin {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<crate::error::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        match tokio::io::AsyncRead::poll_read(self, cx, &mut buf)? {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => Poll::Ready(Ok(buf.filled().len())),
        }
    }
}

impl AsyncWrite for tokio::io::Stdout {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<crate::error::Result<usize>> {
        tokio::io::AsyncWrite::poll_write(self, cx, buf).map_err(Into::into)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<crate::error::Result<()>> {
        tokio::io::AsyncWrite::poll_flush(self, cx).map_err(Into::into)
    }
}

#[cfg(windows)]
mod windows {
    use std::task::Poll;

    use tokio::io::ReadBuf;

    use crate::{
        core::io::{AsyncRead, AsyncWrite},
        error,
        pty::{self, OpenPty, OpenedPty, PtyBuilder},
    };

    struct WithTokio;

    impl PtyBuilder {
        pub fn build(self) -> error::Result<OpenedPty> {
            self.build_with::<WithTokio>()
        }
    }

    impl OpenPty for WithTokio {
        fn connect_input(input: String) -> error::Result<crate::pty::AbstractPtyInput> {
            tokio::net::windows::named_pipe::ClientOptions::new()
                .open(input)
                .map(pty::AbstractPtyInput::new)
                .map_err(Into::into)
        }

        fn connect_output(output: String) -> error::Result<crate::pty::AbstractPtyOutput> {
            tokio::net::windows::named_pipe::ClientOptions::new()
                .open(output)
                .map(pty::AbstractPtyOutput::new)
                .map_err(Into::into)
        }
    }

    impl AsyncRead for tokio::net::windows::named_pipe::NamedPipeClient {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<crate::error::Result<usize>> {
            let mut buf = ReadBuf::new(buf);
            match tokio::io::AsyncRead::poll_read(self, cx, &mut buf)? {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => Poll::Ready(Ok(buf.filled().len())),
            }
        }
    }

    impl AsyncWrite for tokio::net::windows::named_pipe::NamedPipeClient {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<crate::error::Result<usize>> {
            tokio::io::AsyncWrite::poll_write(self, cx, buf).map_err(Into::into)
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<crate::error::Result<()>> {
            tokio::io::AsyncWrite::poll_flush(self, cx).map_err(Into::into)
        }
    }
}
