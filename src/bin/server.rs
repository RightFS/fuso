use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    time::Duration,
};

use bincode::de;
use fuso::{
    config::{client::WithForwardService, server::Config, Stateful},
    core::{
        accepter::{AccepterExt, MultiAccepter, StreamAccepter, TaggedAccepter},
        future::Select,
        handshake::Handshaker,
        io::{AsyncReadExt, AsyncWriteExt, StreamExt},
        net::TcpListener,
        processor::{IProcessor, Processor, StreamProcessor},
        protocol::{AsyncPacketRead, AsyncPacketSend},
        split::SplitStream,
        stream::{
            fallback::Fallback,
            handshake::{ClientConfig, ForwardConfig, Handshake},
            UseCompress, UseCrypto,
        },
        BoxedStream, Stream,
    },
    error,
    runtime::tokio::TokioRuntime,
    server::port_forward::{ForwardAccepter, MuxAccepter, PortForwarder},
};

#[derive(Debug, Clone)]
pub enum Tagged {
    Kcp,
    Tcp,
}

#[cfg(feature = "fuso-cli")]
fn main() {
    match fuso::cli::server::parse() {
        Ok(conf) => fuso::enter_async_main(enter_fuso_main(conf)).unwrap(),
        Err(e) => {
            println!("{:?}", e)
        }
    }
}

async fn enter_fuso_main(conf: Config) -> error::Result<()> {
    //   conf.listens.len()
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    enter_fuso_serve(Stateful::new(conf)).await?;
    // axum::serve(tcp_listener, make_service)

    loop {

        // mgr.manage(|ctrl|async move{

        // });

        // mgr.service(|mgr| async move {
        //     enter_port_forward_service().await
        // });

        // let a = processor.process(a).await;

        // let a = PortForward::new(a, ShareAccepter::new(
        //     accepter,
        //     // Rc4Recognizer(prefix, a)
        // ));

        // let (from, to) = a.accept().await?;

        // let n = reader.read(&mut buf).await?;

        // let (k, s) = a.into_inner();

        // assert_eq!(k, Some(buf[..n].to_vec()));

        // println!("{:?}", String::from_utf8_lossy(&buf[..n]));

        // println!("{:?} => {}", tag, addr);
    }

    Ok(())
}

// pub async fn enter_port_forward_service(mgr){
//     let (tag, (addr, transport)) = accepter.accept().await?;

//     let mut forwarder = PortForwarder::new(
//         transport,
//         {
//             ShareAccepter::new(1110, rand::random(), {
//                 StreamAccepter::new({
//                     fuso::core::net::TcpListener::bind(
//                         SocketAddr::from_str("0.0.0.0:9999").unwrap(),
//                     )
//                     .await?
//                 })
//             })
//         },
//         (),
//         None,
//     );

//     let mgr = 0;

//     let (c1, c2) = forwarder.accept().await?;

//     mgr.spawn(PortForward(), async move{
//         c1.copy(c2);
//     });

//     unimplemented!()
// }

async fn enter_fuso_serve(conf: Stateful<Config>) -> error::Result<()> {
    let mut accepter = MultiAccepter::new();

    for listen in &conf.listens {
        match listen {
            fuso::config::server::Listen::Kcp(kcp) => {
                accepter.add(TaggedAccepter::new(
                    Tagged::Kcp,
                    StreamAccepter::new({
                        fuso::core::net::KcpListener::bind(Default::default(), kcp.as_socket_addr())
                            .await?
                    }),
                ));
            }
            fuso::config::server::Listen::Tcp(tcp) => {
                accepter.add(TaggedAccepter::new(
                    Tagged::Tcp,
                    StreamAccepter::new({
                        fuso::core::net::TcpListener::bind(tcp.as_socket_addr()).await?
                    }),
                ));
            }
            fuso::config::server::Listen::Proxy(proxy) => {
                accepter.add(TaggedAccepter::new(
                    Tagged::Tcp,
                    StreamAccepter::new({
                        fuso::core::net::TcpListener::bind(proxy.as_socket_addr()).await?
                    }),
                ));
            }
            fuso::config::server::Listen::Tunnel(forward) => {
                accepter.add(TaggedAccepter::new(
                    Tagged::Tcp,
                    StreamAccepter::new({
                        fuso::core::net::TcpListener::bind(forward.as_socket_addr()).await?
                    }),
                ));
            }
        }
    }

    loop {
        let (_, (addr, transport)) = accepter.accept().await?;

        log::debug!("connection from {addr}");
        let conf = conf.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(conf, transport).await {
                log::error!("{:?}", e);
            }
        });
    }
}

pub async fn handle_connection(
    conf: Stateful<Config>,
    stream: BoxedStream<'static>,
) -> error::Result<()> {
    let crypto = &conf.crypto;
    let compress = &conf.compress;

    let mut stream = stream
        .use_crypto(crypto.iter())
        .use_compress(compress.iter())
        .server_handshake::<TokioRuntime>(
            &conf.authentication,
            Duration::from_secs(conf.authentication_timeout as _),
        )
        .await?;

    match stream.read_config().await? {
        ClientConfig::Forward(forward) => enter_forwarder(forward, stream).await?,
    }

    Ok(())
}

pub async fn enter_forwarder<S>(conf: ForwardConfig, transport: S) -> error::Result<()>
where
    S: Stream + Send + Unpin + 'static,
{
    log::debug!("{:#?}", conf);

    let mut accepter = MultiAccepter::new();

    if let Some(port) = conf.channel {
        accepter.add(ForwardAccepter::new(
            TcpListener::bind(format!("0:{port}")).await?,
        ));
    }

    let mut multi_mux_accepter = MultiAccepter::new();

    for port in conf.exposes.iter() {
        multi_mux_accepter.add(StreamAccepter::new(
            TcpListener::bind(format!("0:{port}")).await?,
        ));
    }

    let mux_accepter = MuxAccepter::new(1110, rand::random(), {
        StreamAccepter::new(multi_mux_accepter)
    });

    accepter.add(mux_accepter);

    let mut forwarder = PortForwarder::new(transport, accepter, (), None);

    log::debug!("forward started ..");

    loop {
        let (c1, c2) = forwarder.accept().await?;
    }


    Ok(())
}
