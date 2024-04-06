use std::{net::SocketAddr, time::Duration};

use fuso::{
    cli,
    client::port_forward::PortForwarder,
    config::{
        client::{
            Config, Host, ServerAddr, Service, WithBridgeService, WithForwardService,
            WithProxyService,
        },
        Stateful,
    },
    core::{
        accepter::AccepterExt,
        connector::MultiConnector,
        io::{AsyncReadExt, StreamExt},
        net::{TcpListener, TcpStream},
        protocol::{AsyncPacketRead, AsyncPacketSend},
        rpc::{AsyncCall, Caller},
        stream::{handshake::Handshake, UseCompress, UseCrypto},
    },
    error,
    runner::{FnRunnable, NamedRunnable, Rise, ServiceRunner},
    runtime::{tokio::TokioRuntime, Runtime},
};

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

async fn enter_forward_service_main(
    config: Stateful<Config>,
    service: WithForwardService,
) -> error::Result<Rise> {
    let server = &config.server;
    let crypto = &server.crypto;
    let compress = &server.compress;

    let result = server
        .try_connect(|host, port| async move {
            log::debug!("connect to server {host}:{port}");
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

    let mut stream = match result {
        Ok(stream) => stream,
        Err(e) => {
            log::error!("fail to handshake {e:?}");
            return Ok(Rise::Restart);
        }
    };

    stream.write_config(&service).await?;

    let mut connector = MultiConnector::<usize, usize>::new();

    if let Some(channel) = service.channel.as_ref() {
        // connector.add(connector)
    }

    let mut forwarder = PortForwarder::new(stream, connector);

    loop {
        let (linker, target) = forwarder.accept().await?;

        tokio::spawn(async move {

            
            let a = linker.link().await;
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
                    upstream.copy(&mut stream).await;
                }
                Err(e) => {}
            };
        });
    }
}
