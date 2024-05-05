use crate::{
    core::{processor::Preprocessor, rpc::structs::port_forward::VisitorProtocol},
    error,
};

use super::Connection;


pub struct Socks5Preprocessor {
    udp_forward: bool,
}


impl Preprocessor<Connection> for () {
    type Output = error::Result<VisitorProtocol>;

    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        Box::pin(async move { Ok(VisitorProtocol::Other(input, None)) })
    }
}

impl Preprocessor<Connection> for Option<()> {
    type Output = error::Result<Connection>;
    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        Box::pin(async move { Ok(input) })
    }
}


impl Preprocessor<Connection> for Socks5Preprocessor {
    type Output = error::Result<VisitorProtocol>;
    fn prepare<'a>(&'a self, input: Connection) -> crate::core::BoxedFuture<'a, Self::Output> {
        unimplemented!()
    }
}
