use std::{net::SocketAddr, sync::Arc, time::Duration};

use fuso::{
    cli,
    client::port_forward::{MuxConnector, PortForwarder, Protocol},
    config::{
        client::{
            Config, FinalTarget, Host, ServerAddr, Service, WithBridgeService, WithForwardService,
            WithProxyService,
        },
        Compress, Crypto, Expose, Stateful,
    },
    core::{
        accepter::AccepterExt,
        connector::MultiConnector,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        protocol::{AsyncPacketRead, AsyncPacketSend},
        rpc::{structs::port_forward::Target, AsyncCall, Caller, Decoder, Encoder},
        stream::{
            compress::CompressedStream,
            fragment::Fragment,
            handshake::{Handshake, MuxConfig},
            UseCompress, UseCrypto,
        },
        transfer::{AbstractTransmitter, TransmitterExt},
        AbstractStream, Connection,
    },
    error,
    runner::{FnRunnable, NamedRunnable, Rise, ServiceRunner},
    runtime::{tokio::TokioRuntime, Runtime},
};
use kcp_rust::KcpConnector;

fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();

    match cli::client::parse() {
        Ok(conf) => {
            conf.check();
            fuso::enter_async_main(enter_fuso_main(conf)).unwrap()
        }
        Err(e) => {
            log::error!("{e:?}")
        }
    }
}

fn enter_fuso_main(mut conf: Config) -> ServiceRunner<'static, TokioRuntime> {
    let mut sp = ServiceRunner::<TokioRuntime>::new();

    let services = std::mem::replace(&mut conf.services, Default::default());

    let conf = Stateful::new(conf);

    for (name, service) in services {
        let sc = conf.clone();
        match service {
            Service::Proxy(s) => {
                sp.register(
                    s.restart.clone(),
                    NamedRunnable::new(name, {
                        FnRunnable::new(move || enter_proxy_service_main(sc.clone(), s.clone()))
                    }),
                );
            }
            Service::Bridge(s) => {
                sp.register(
                    s.restart.clone(),
                    NamedRunnable::new(name, {
                        FnRunnable::new(move || enter_bridge_service_main(sc.clone(), s.clone()))
                    }),
                );
            }
            Service::Forward(s) => {
                sp.register(
                    s.restart.clone(),
                    NamedRunnable::new(name, {
                        FnRunnable::new(move || enter_forward_service_main(sc.clone(), s.clone()))
                    }),
                );
            }
        }
    }

    sp.build()
}

async fn try_tcp_connect<'a, Cr, Co>(
    addr: ServerAddr,
    port: u16,
    cryptos: Cr,
    compress: Co,
) -> error::Result<CompressedStream<'static>>
where
    Cr: Iterator<Item = &'a Crypto>,
    Co: Iterator<Item = &'a Compress>,
{
    addr.try_connect(port, |host, port| async move {
        match host {
            Host::Ip(ip) => TcpStream::connect(SocketAddr::new(*ip, port)).await,
            Host::Domain(domain) => TcpStream::connect(format!("{domain}:{port}")).await,
        }
    })
    .await
    .map(|stream| stream.use_crypto(cryptos).use_compress(compress))
}

async fn enter_forward_service_main(
    config: Stateful<Config>,
    service: WithForwardService,
) -> error::Result<Rise> {
    let server = &config.server;
    let crypto = &server.crypto;
    let compress = &server.compress;

    let result = server
        .try_connect(|host, port| async move {
            log::trace!("connect to server {host}:{port}");
            let connection = match host {
                Host::Ip(ip) => TcpStream::connect(SocketAddr::new(*ip, port)).await,
                Host::Domain(domain) => TcpStream::connect(format!("{domain}:{port}")).await,
            };

            connection.map(|c| {
                log::debug!("connection established: {host}:{port}");
                c
            })
        })
        .await?
        .use_crypto(crypto.iter())
        .use_compress(compress.iter())
        .client_handshake::<TokioRuntime>(
            &server.authentication,
            Duration::from_secs(server.authentication_timeout as _),
        )
        .await;

    let mut stream = match result {
        Ok(stream) => stream,
        Err(e) => {
            log::error!("fail to handshake {e:?}");
            return Ok(Rise::Restart);
        }
    };

    stream.write_config(&service).await?;

    let mut connector = MultiConnector::<Protocol, AbstractTransmitter<'static>>::new();

    if let Some(channels) = service.channel.as_ref() {
        for expose in channels.iter() {
            match expose {
                Expose::Kcp(_, port) => {}
                Expose::Tcp(_, port) => {
                    let port = *port;
                    let cryptos = service.crypto.clone();
                    let compress = service.compress.clone();
                    let addr = server.addr.clone();
                    connector.add(move |proto| {
                        let addr = addr.clone();
                        let cryptos = cryptos.clone();
                        let compress = compress.clone();
                        async move {
                            try_tcp_connect(addr, port, cryptos.iter(), compress.iter())
                                .await
                                .map(Into::into)
                        }
                    })
                }
            }
        }
    } else {
        let mux: MuxConfig = MuxConfig::default();

        stream.send_packet(&mux.borrow_encode()?).await?;

        for expose in service.exposes.clone() {
            let addr = server.addr.clone();

            match expose {
                Expose::Kcp(_, port) => todo!(),
                Expose::Tcp(_, port) => {
                    connector.add(MuxConnector::new(mux.clone(), move |proto| {
                        let addr = addr.clone();
                        async move {
                            addr.try_connect(port, |host, port| async move {
                                let connection = match host {
                                    Host::Ip(ip) => {
                                        TcpStream::connect(SocketAddr::new(*ip, port)).await
                                    }
                                    Host::Domain(domain) => {
                                        TcpStream::connect(format!("{domain}:{port}")).await
                                    }
                                }?;

                                Ok(AbstractStream::new(connection).into())
                            })
                            .await
                        }
                    }))
                }
            }
        }
    }

    let mut forwarder = PortForwarder::new(stream, service.target, connector);

    log::debug!("forward started .");

    loop {
        let (linker, target) = forwarder.accept().await?;
        tokio::spawn(async move {
            log::debug!("target {:?}", target);

            match target {
                FinalTarget::Udp { addr, port } => {}
                FinalTarget::Shell { path, args } => todo!(),
                FinalTarget::Dynamic => {
                    let transmitter = linker.link(Protocol::Tcp).await.unwrap();

                    let udp = Arc::new(tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap());

                    let (writer, mut reader) = transmitter.split();

                    loop {
                        let pkt = match reader.recv_packet().await {
                            Err(_) => break,
                            Ok(data) => data,
                        };

                        let pkt: Fragment = pkt.decode().unwrap();

                        let udp = udp.clone();
                        let mut writer = writer.clone();

                        match pkt {
                            Fragment::UdpForward {
                                saddr,
                                daddr,
                                dport,
                                data,
                            } => {
                                let target = format!("{}:{dport}", daddr.to_string());
                                log::debug!("udp forward {saddr} -> {target}");
                                udp.send_to(&data, target).await.unwrap();
                                let mut buf = [0u8; 1400];
                                let (n, addr) = udp.recv_from(&mut buf).await.unwrap();

                                writer
                                    .send_packet(
                                        &Fragment::Udp(saddr, buf[..n].to_vec()).encode().unwrap(),
                                    )
                                    .await
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                }
                FinalTarget::Tcp { addr, port } => {
                    let result = addr
                        .try_connect(port, |host, port| async move {
                            match host {
                                Host::Ip(ip) => {
                                    TcpStream::connect(SocketAddr::new(*ip, port)).await
                                }
                                Host::Domain(domain) => {
                                    TcpStream::connect(format!("{domain}:{port}")).await
                                }
                            }
                        })
                        .await;

                    match result {
                        Ok(stream) => match linker.link(Protocol::Tcp).await {
                            Ok(transmitter) => {
                                transmitter.transfer(stream).await;
                            }
                            Err(e) => {
                                log::debug!("{:?}", e);
                            }
                        },
                        Err(e) => {
                            if let Err(e) = linker.cancel(e).await {
                                log::warn!("{:?}", e);
                            };
                        }
                    }
                }
            }
        });
    }
}

async fn enter_proxy_service_main(
    config: Stateful<Config>,
    service: WithProxyService,
) -> error::Result<Rise> {
    let server = &config.server;
    let crypto = &server.crypto;
    let compress = &server.compress;

    let result = server
        .try_connect(|host, port| async move {
            // log::debug!("connect to server {host}:{port}");
            match host {
                Host::Ip(ip) => TcpStream::connect(SocketAddr::new(*ip, port)).await,
                Host::Domain(domain) => TcpStream::connect(format!("{domain}:{port}")).await,
            }
        })
        .await?
        .use_crypto(crypto.iter())
        .use_compress(compress.iter())
        .client_handshake::<TokioRuntime>(
            &server.authentication,
            Duration::from_secs(server.authentication_timeout as _),
        )
        .await;

    let transport = match result {
        Ok(stream) => stream,
        Err(e) => {
            log::error!("fail to handshake {e:?}");
            return Ok(Rise::Fatal);
        }
    };

    Ok(Rise::Restart)
}

async fn enter_bridge_service_main(
    config: Stateful<Config>,
    service: WithBridgeService,
) -> error::Result<Rise> {
    let mut listener = match TcpListener::bind(SocketAddr::new(service.bind, service.port)).await {
        Ok(listener) => listener,
        Err(e) => {
            return Ok(Rise::Fatal);
        }
    };

    log::debug!("bridge started ... {}:{}", service.bind, service.port);

    loop {
        let (addr, mut stream) = listener.accept().await?;

        log::debug!("{:?}", addr);

        let server = config.server.clone();

        tokio::spawn(async move {
            let result = server
                .try_connect(|host, port| async move {
                    match host {
                        Host::Ip(ip) => TcpStream::connect(SocketAddr::new(*ip, port)).await,
                        Host::Domain(domain) => {
                            TcpStream::connect(format!("{domain}:{port}")).await
                        }
                    }
                })
                .await;

            match result {
                Ok(upstream) => {
                    upstream.transfer(stream).await;
                }
                Err(e) => {}
            };
        });
    }
}
