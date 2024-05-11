mod c;

use std::{collections::HashMap, ffi::CString};

use c::bindings;

use crate::{
    core::io::{AsyncRead, AsyncWrite},
    error,
};

extern "C" {
    pub fn read(fd: i32, buf: *mut u8, nbyte: usize) -> isize;
}

#[derive(Debug)]
struct Process {
    args: Vec<*const u8>,
    envp: Vec<*const u8>,
    work_dir: *const u8,
}

#[derive(Debug)]
pub struct Pty {
    pty: bindings::pty_process,
    proc: Process,
    work: Option<CString>,
    argv: Vec<CString>,
    envp: Vec<CString>,
}

impl Pty {
    fn open(
        prog: String,
        work: Option<String>,
        args: Option<Vec<String>>,
        envp: Option<HashMap<String, String>>,
    ) -> error::Result<Self> {
        unsafe {
            let mut pty = std::mem::zeroed::<bindings::pty_process>();

            let work = match work {
                Some(work) => Some(CString::new(work)?),
                None => None,
            };

            let mut argv = Vec::<CString>::new();

            argv.push(CString::new(prog)?);

            if let Some(args) = args {
                for arg in args {
                    argv.push(CString::new(arg)?);
                }
            }

            let envp = match envp {
                None => Vec::new(),
                Some(envp) => envp.into_iter().fold(Vec::new(), |mut envp, (k, v)| {
                    envp.push(CString::new(format!("{k}={v}")).unwrap());
                    envp
                }),
            };

            let proc = Process {
                args: {
                    let mut args = vec![];
                    args.extend(argv.iter().map(|arg| arg.as_ptr() as *const u8));
                    args.push(std::ptr::null());
                    args
                },
                envp: {
                    let mut envs = vec![];
                    envs.extend(envp.iter().map(|env| env.as_ptr() as *const u8));
                    envs.push(std::ptr::null());
                    envs
                },
                work_dir: {
                    work.as_ref()
                        .map(|work| work.as_ptr() as _)
                        .unwrap_or(std::ptr::null())
                },
            };

            pty.argv = proc.args.as_ptr() as _;
            pty.envp = proc.envp.as_ptr() as _;
            pty.work = proc.work_dir as _;

            let error = bindings::pty_spawn(&mut pty as *mut bindings::pty_process);

            if error < 0 {
                Err(std::io::Error::from_raw_os_error(error).into())
            } else {
                Ok(Self {
                    pty,
                    proc,
                    work,
                    argv,
                    envp,
                })
            }
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe { bindings::pty_exit(&mut self.pty as *mut bindings::pty_process) }
    }
}

impl AsyncRead for Pty {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<error::Result<usize>> {
        unimplemented!()
    }
}

impl AsyncWrite for Pty {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<error::Result<usize>> {
        unimplemented!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<error::Result<()>> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::FromRawFd;

    use mio::{unix::SourceFd, Events, Interest, Token};
    use tokio::io::AsyncReadExt;

    use super::Pty;

    #[tokio::test]
    async fn test_pty() {
        let pty = Pty::open("/bin/sh".to_owned(), None, None, None).unwrap();

        let poll = mio::Poll::new().unwrap();

        poll.registry();

        println!("{pty:?} {:?}", pty.argv)
    }
}
