pub mod port_forward {
    use serde::{Deserialize, Serialize};

    use crate::{config::client::ServerAddr, core::Connection};

    #[derive(Debug, Serialize, Deserialize)]
    pub enum Target {
        Udp(ServerAddr),
        Tcp(ServerAddr),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum Request {
        New(u64, Option<Target>),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum Response {
        Ok,
        Error(),
    }

    pub enum WithSocks {
        Tcp(ServerAddr),
        Udp(ServerAddr),
    }

    pub enum VisitorProtocol {
        Socks(Connection<'static>, WithSocks),
        Other(Connection<'static>, Option<ServerAddr>),
    }
}
