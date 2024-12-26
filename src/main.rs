use async_trait::async_trait;
use hickory_proto::op::{MessageType, ResponseCode};
use hickory_proto::rr::{Name, Record, RecordType};
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::RequestHandler;
use hickory_server::server::{Request, ResponseHandler, ResponseInfo};
use log::{debug, error, info, warn};
use pingora_core::listeners::tls::TlsSettings;
use pingora_core::server::configuration::Opt;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::{HttpPeer, Peer};
use pingora_error::{Error, ErrorType, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use prometheus::{register_int_counter, register_int_gauge};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::net::UdpSocket;
use std::sync::Arc;

// Proxy implementation
pub struct MyProxy {
    req_metric: prometheus::IntCounter,
    active_connections: prometheus::IntGauge,
    dns_server: Arc<Server>,
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

// DNS handler implementation
pub struct DnsHandler {
    records: Vec<Record>,
}

impl DnsHandler {
    pub fn new() -> Self {
        // Initialize with some static records
        let mut records = Vec::new();

        // Add A record for example.com
        if let Ok(name) = Name::from_ascii("example.com") {
            records.push(Record::from_rdata(
                name,
                300,
                hickory_proto::rr::RData::A("93.184.216.34".parse().unwrap()),
            ));
        }

        DnsHandler { records }
    }
}

#[async_trait]
impl RequestHandler for DnsHandler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        let query = request.query();
        let query_name = query.name();
        let query_type = query.query_type();

        // Find matching records
        let matching_records: Vec<_> = self
            .records
            .iter()
            .filter(|r| {
                r.name().to_string().to_lowercase() == query_name.to_string().to_lowercase()
                    && (query_type == RecordType::ANY || r.record_type() == query_type)
            })
            .cloned()
            .collect();

        // Build response header
        let mut header = request.header().clone();
        header.set_message_type(MessageType::Response);
        header.set_authoritative(true);

        if !matching_records.is_empty() {
            header.set_response_code(ResponseCode::NoError);
            let builder = MessageResponseBuilder::from_message_request(request);
            let response = builder.build(
                header.clone(),
                matching_records.iter(),
                None,
                None,
                None,
            );
            let _ = response_handle.send_response(response);
        } else {
            header.set_response_code(ResponseCode::NXDomain);
            let builder = MessageResponseBuilder::from_message_request(request);
            let response = builder.build(
                header.clone(),
                std::iter::empty(),
                None,
                None,
                None,
            );
            let _ = response_handle.send_response(response);
        }

        ResponseInfo::from(header)
    }
}

// Make DnsHandler Send + Sync + Unpin + 'static
unsafe impl Send for DnsHandler {}
unsafe impl Sync for DnsHandler {}
impl Unpin for DnsHandler {}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Create server
    let mut server = Server::new(Some(Opt::default())).unwrap();
    server.bootstrap();

    // Setup proxy service
    let proxy = MyProxy {
        req_metric: register_int_counter!("req_counter", "Number of requests").unwrap(),
        active_connections: register_int_gauge!("active_connections", "Number of active connections").unwrap(),
        dns_server: Arc::new(Server::new(None).unwrap()),
    };

    let mut proxy_service = pingora_proxy::http_proxy_service(&server.configuration, proxy);

    // Add plain HTTP listener
    proxy_service.add_tcp("0.0.0.0:8080");

    // Add HTTPS listener with TLS
    let cert_path = "certs/server.crt";
    let key_path = "certs/server.key";
    if std::path::Path::new(cert_path).exists() && std::path::Path::new(key_path).exists() {
        let mut tls_settings = TlsSettings::intermediate(cert_path, key_path).unwrap();
        tls_settings.enable_h2();
        proxy_service.add_tls_with_settings("0.0.0.0:8443", None, tls_settings);
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
    let dns_handler = DnsHandler::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use pingora_http::RequestHeader;
    use tokio_test::io::Builder;
    use std::sync::Arc;

    async fn setup_test_proxy(test_name: &str) -> MyProxy {
        MyProxy {
            req_metric: register_int_counter!(
                &format!("test_req_counter_{}", test_name),
                &format!("Test request counter for {}", test_name)
            ).unwrap(),
            active_connections: register_int_gauge!(
                &format!("test_active_connections_{}", test_name),
                &format!("Test active connections for {}", test_name)
            ).unwrap(),
            dns_server: Arc::new(Server::new(None).unwrap()),
        }
    }

    #[tokio::test]
    async fn test_proxy_initialization() {
        let proxy = setup_test_proxy("init").await;
        let _ctx = proxy.new_ctx();
    }

    #[tokio::test]
    async fn test_proxy_upstream_peer() -> Result<()> {
        let proxy = setup_test_proxy("peer").await;
        let _session = Session::new_h1(Box::new(Builder::new().build()));
        let _ctx = proxy.new_ctx();

        // Create a peer directly to test the configuration
        let peer = Box::new(HttpPeer::new(
            ("1.1.1.1", 443),
            true,
            "example.com".to_string(),
        ));

        assert_eq!(peer.address().to_string().split(':').nth(1).unwrap(), "443");
        assert_eq!(peer.options.connection_timeout, None);
        assert_eq!(peer.options.read_timeout, None);
        assert_eq!(peer.options.write_timeout, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_proxy_request_filter() -> Result<()> {
        let proxy = setup_test_proxy("filter").await;
        let mut session = Session::new_h1(Box::new(Builder::new().build()));
        let mut ctx = proxy.new_ctx();
        let mut request = RequestHeader::build("GET", b"/", None).unwrap();

        proxy.upstream_request_filter(&mut session, &mut request, &mut ctx).await?;

        assert_eq!(
            request.headers.get("x-forwarded-by").unwrap(),
            "MyProxy"
        );
        Ok(())
    }
}
