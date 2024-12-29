use std::sync::Arc;
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use std::path::PathBuf;
use log::{debug, error, info, warn};
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo, Protocol};
use hickory_server::authority::{MessageResponse, MessageResponseBuilder};
use prometheus::{register_int_counter, register_int_gauge};
use pingora_core::server::configuration::Opt;
use pingora_core::server::Server;
use pingora_core::listeners::tls::TlsSettings;
use pingora_core::upstreams::peer::{HttpPeer, Peer};
use pingora_error::{Error, ErrorType, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use tokio::net::UdpSocket;
use async_trait::async_trait;
use std::io;

mod driver;
mod registry;
mod site;
mod dns;

use crate::driver::LaravelDriver;
use crate::registry::DriverRegistry;
use crate::site::SiteManager;

// Proxy implementation
pub struct MyProxy {
    req_metric: prometheus::IntCounter,
    active_connections: prometheus::IntGauge,
    dns_server: Arc<Server>,
    site_manager: Arc<SiteManager>,
}

#[async_trait]
impl ProxyHttp for MyProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // Get host from request header
        let host = session
            .req_header()
            .headers
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("one.one.one.one");

        // Default to 1.1.1.1 as upstream
        let mut peer = Box::new(HttpPeer::new(
            ("1.1.1.1", 443),
            true,
            host.to_string(),
        ));

        // Configure timeouts
        peer.options.connection_timeout = Some(Duration::from_secs(10));
        peer.options.read_timeout = Some(Duration::from_secs(30));
        peer.options.write_timeout = Some(Duration::from_secs(30));

        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Add any custom headers
        upstream_request
            .insert_header("X-Forwarded-By", "MyProxy")
            .map_err(|_| Error::new(ErrorType::InvalidHTTPHeader))?;
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Add custom response headers
        upstream_response
            .insert_header("Server", "MyProxy")
            .map_err(|_| Error::new(ErrorType::InvalidHTTPHeader))?;
        Ok(())
    }

    async fn logging(
        &self,
        session: &mut Session,
        error: Option<&pingora_core::Error>,
        _ctx: &mut Self::CTX,
    ) {
        let response_code = session
            .response_written()
            .map_or(0, |resp| resp.status.as_u16());

        if let Some(e) = error {
            error!(
                "Request failed: {} response_code: {} error: {}",
                self.request_summary(session, _ctx),
                response_code,
                e
            );
        } else {
            info!(
                "{} response_code: {}",
                self.request_summary(session, _ctx),
                response_code
            );
        }

        self.req_metric.inc();
    }

    async fn connected_to_upstream(
        &self,
        _session: &mut Session,
        reused: bool,
        peer: &HttpPeer,
        #[cfg(unix)] _fd: std::os::unix::io::RawFd,
        #[cfg(windows)] _sock: std::os::windows::io::RawSocket,
        _digest: Option<&pingora_core::protocols::Digest>,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        debug!(
            "Connected to upstream {} (reused: {})",
            peer.address().to_string(),
            reused
        );
        self.active_connections.inc();
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Create server
    let mut server = Server::new(Some(Opt::default())).unwrap();
    server.bootstrap();

    // Initialize site manager and driver registry
    let mut registry = DriverRegistry::new();
    let site_manager = Arc::new(SiteManager::new());

    // Register Laravel driver with default PHP version
    registry.register(Arc::new(LaravelDriver::new(
        PathBuf::from("/"),  // Base path will be set per site
        "8.2".to_string(),   // Default PHP version
    )));

    // Setup proxy service
    let proxy = MyProxy {
        req_metric: register_int_counter!("req_counter", "Number of requests").unwrap(),
        active_connections: register_int_gauge!("active_connections", "Number of active connections").unwrap(),
        dns_server: Arc::new(Server::new(None).unwrap()),
        site_manager: site_manager.clone(),
    };

    let mut proxy_service = pingora_proxy::http_proxy_service(&server.configuration, proxy);

    // Add plain HTTP listener
    proxy_service.add_tcp("0.0.0.0:80");

    // Add HTTPS listener with TLS
    let cert_path = "certs/server.crt";
    let key_path = "certs/server.key";
    if std::path::Path::new(cert_path).exists() && std::path::Path::new(key_path).exists() {
        let mut tls_settings = TlsSettings::intermediate(cert_path, key_path).unwrap();
        tls_settings.enable_h2();
        proxy_service.add_tls_with_settings("0.0.0.0:443", None, tls_settings);
    } else {
        warn!("TLS certificates not found, HTTPS listener disabled");
    }

    // Add prometheus metrics endpoint
    let mut prometheus_service = pingora_core::services::listening::Service::prometheus_http_service();
    prometheus_service.add_tcp("127.0.0.1:9090");

    // Add services to server
    server.add_service(proxy_service);
    server.add_service(prometheus_service);

    // Start DNS server
    let dns_handler = dns::DnsHandler::new();
    let mut dns_server = hickory_server::ServerFuture::new(dns_handler);

    match UdpSocket::bind("0.0.0.0:53").await {
        Ok(socket) => {
            dns_server.register_socket(socket);
            info!("DNS server listening on 0.0.0.0:53");
        }
        Err(e) => {
            error!("Failed to bind DNS server to port 53: {}", e);
            // Continue without DNS server
        }
    }

    // Run both servers
    let proxy_future = tokio::spawn(async move {
        let server_future: Pin<Box<dyn Future<Output = Result<()>> + Send>> = Box::pin(async move {
            server.run_forever()
        });
        if let Err(e) = server_future.await {
            error!("HTTP proxy server error: {}", e);
        }
        info!("HTTP proxy server stopped");
    });

    let dns_future = tokio::spawn(async move {
        dns_server.block_until_done().await;
        info!("DNS server stopped");
    });

    tokio::select! {
        _ = proxy_future => {}
        _ = dns_future => {}
    }

    Ok(())
}
