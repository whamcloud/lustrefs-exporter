// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use clap::Parser;
use lustrefs_exporter::{Error, dump_stats, routes::app};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore};

const LUSTREFS_EXPORTER_PORT: &str = "32221";

/// Maximum number of concurrent TCP connections.
const MAX_CONNECTIONS: usize = 512;

/// Absolute maximum lifetime for any single TCP connection (in seconds).
const MAX_CONNECTION_LIFETIME_SECS: u64 = 900;

/// Timeout for reading HTTP request headers.
const HEADER_READ_TIMEOUT_SECS: u64 = 30;

/// TCP keepalive idle time (seconds before first probe).
const TCP_KEEPALIVE_TIME_SECS: u64 = 60;

/// TCP keepalive probe interval (seconds between probes).
const TCP_KEEPALIVE_INTERVAL_SECS: u64 = 10;

#[derive(Debug, Parser)]
pub struct CommandOpts {
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,

    #[clap(long, hide = true)]
    dump: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    if opts.dump {
        dump_stats().await?;
    } else {
        let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));
        tracing::info!("Listening on http://{addr}/metrics");

        let listener = TcpListener::bind(addr).await?;
        serve(listener, app()).await?;
    }

    Ok(())
}

async fn serve(listener: TcpListener, app: axum::Router) -> Result<(), Error> {
    use axum::extract::Request;
    use hyper::body::Incoming;
    use hyper_util::{
        rt::{TokioExecutor, TokioIo, TokioTimer},
        server::conn::auto,
        service::TowerToHyperService,
    };
    use tower::ServiceExt as _;

    let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

    let mut base_builder = auto::Builder::new(TokioExecutor::new());
    base_builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(HEADER_READ_TIMEOUT_SECS));

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to accept connection: {e}");
                continue;
            }
        };

        let permit = match semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!(
                    "Connection limit ({MAX_CONNECTIONS}) reached, rejecting connection from {peer_addr}",
                );
                drop(stream);
                continue;
            }
        };

        // Enable TCP keepalive on this socket to detect dead peers.
        if let Err(e) = set_tcp_keepalive(&stream) {
            tracing::warn!("Failed to set TCP keepalive for {peer_addr}: {e}");
        }

        let app_clone = app.clone();
        let builder = base_builder.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let tower_service = app_clone
                .clone()
                .map_request(|req: Request<Incoming>| req.map(axum::body::Body::new));
            let hyper_service = TowerToHyperService::new(tower_service);

            let conn = builder.serve_connection(io, hyper_service);

            match tokio::time::timeout(Duration::from_secs(MAX_CONNECTION_LIFETIME_SECS), conn)
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::debug!("Connection error from {peer_addr}: {e}");
                }
                Err(_) => {
                    tracing::debug!(
                        "Connection from {peer_addr} exceeded max lifetime ({}s), closing",
                        MAX_CONNECTION_LIFETIME_SECS,
                    );
                }
            }

            drop(permit);
        });
    }
}

/// Set TCP keepalive on an accepted socket to detect dead peers.
fn set_tcp_keepalive(stream: &tokio::net::TcpStream) -> std::io::Result<()> {
    use socket2::{SockRef, TcpKeepalive};

    let socket = SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(TCP_KEEPALIVE_TIME_SECS))
        .with_interval(Duration::from_secs(TCP_KEEPALIVE_INTERVAL_SECS));
    socket.set_tcp_keepalive(&keepalive)
}
