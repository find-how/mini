use std::sync::Arc;
use anyhow::Result;
use pingora::prelude::*;
use pingora::http::{RequestHeader, Method, StatusCode, ResponseHeader};
use pingora::protocols::http::v1::server::HttpSession;
use crate::site::SiteManager;

pub struct MiniServer {
    site_manager: Arc<SiteManager>,
}

impl MiniServer {
    pub fn new(site_manager: Arc<SiteManager>) -> Self {
        Self { site_manager }
    }

    pub async fn handle_request(&self, session: &mut HttpSession) -> Result<()> {
        let req_header = session.req_header()
            .ok_or_else(|| Error::new(ErrorType::BadRequest))?;

        let host = req_header.host()
            .ok_or_else(|| Error::new(ErrorType::BadRequest))?;

        let uri = req_header.uri()
            .ok_or_else(|| Error::new(ErrorType::BadRequest))?;

        let site = self.site_manager.get_site_from_host(host)
            .ok_or_else(|| Error::new(ErrorType::BadRequest))?;

        let response = site.driver.handle_request(uri).await?;

        let mut resp_header = ResponseHeader::build(StatusCode::OK, None)?;
        session.write_response_header(&resp_header).await?;
        session.write_body_bytes(&response).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use crate::driver::{StaticDriver, LaravelDriver};
    use crate::registry::DriverRegistry;
    use tokio::io::duplex;

    async fn setup_test_server() -> (TempDir, Arc<SiteManager>, MiniServer) {
        let temp_dir = TempDir::new().unwrap();
        let driver_registry = DriverRegistry::new();
        let site_manager = Arc::new(SiteManager::new(driver_registry));
        let server = MiniServer::new(site_manager.clone());
        (temp_dir, site_manager, server)
    }

    #[tokio::test]
    async fn test_static_site_request() {
        let (temp_dir, site_manager, server) = setup_test_server().await;
        let site_path = temp_dir.path().to_path_buf();

        // Create a static site
        fs::write(site_path.join("index.html"), "Hello World").unwrap();
        fs::write(site_path.join("style.css"), "body { color: red }").unwrap();
        fs::create_dir_all(site_path.join("assets")).unwrap();
        fs::write(site_path.join("assets/app.js"), "console.log('Hello')").unwrap();

        Arc::get_mut(&mut site_manager.clone()).unwrap().add_site(
            "example.com".to_string(),
            site_path.clone(),
            Arc::new(StaticDriver::default()),
        );

        let (client, server_stream) = duplex(1024);
        let mut session = HttpSession::new(Box::new(server_stream));

        // Test index request
        let mut req_header = RequestHeader::build(Method::GET, b"/", None)?;
        req_header.insert_header("Host", "example.com")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();

        // Test CSS request
        let mut req_header = RequestHeader::build(Method::GET, b"/style.css", None)?;
        req_header.insert_header("Host", "example.com")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();

        // Test JS request
        let mut req_header = RequestHeader::build(Method::GET, b"/assets/app.js", None)?;
        req_header.insert_header("Host", "example.com")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();
    }

    #[tokio::test]
    async fn test_laravel_site_request() {
        let (temp_dir, site_manager, server) = setup_test_server().await;
        let site_path = temp_dir.path().to_path_buf();

        // Create a Laravel site
        fs::create_dir_all(site_path.join("public")).unwrap();
        fs::write(site_path.join("public/index.php"), "<?php echo 'Hello'; ?>").unwrap();
        fs::write(site_path.join("artisan"), "").unwrap();

        Arc::get_mut(&mut site_manager.clone()).unwrap().add_site(
            "laravel.test".to_string(),
            site_path.clone(),
            Arc::new(LaravelDriver::new(site_path.clone(), "8.2".to_string())),
        );

        let (client, server_stream) = duplex(1024);
        let mut session = HttpSession::new(Box::new(server_stream));

        // Test index request
        let mut req_header = RequestHeader::build(Method::GET, b"/", None)?;
        req_header.insert_header("Host", "laravel.test")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();
    }

    #[tokio::test]
    async fn test_secured_site_request() {
        let (temp_dir, site_manager, server) = setup_test_server().await;
        let site_path = temp_dir.path().to_path_buf();

        // Create a static site
        fs::write(site_path.join("index.html"), "Hello World").unwrap();

        let mut site_manager = site_manager.clone();
        Arc::get_mut(&mut site_manager).unwrap().add_site(
            "secure.test".to_string(),
            site_path.clone(),
            Arc::new(StaticDriver::default()),
        );
        Arc::get_mut(&mut site_manager).unwrap().secure_site("secure.test".to_string()).await.unwrap();

        let (client, server_stream) = duplex(1024);
        let mut session = HttpSession::new(Box::new(server_stream));

        // Test index request
        let mut req_header = RequestHeader::build(Method::GET, b"/", None)?;
        req_header.insert_header("Host", "secure.test")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();
    }

    #[tokio::test]
    async fn test_parked_site_request() {
        let (temp_dir, site_manager, server) = setup_test_server().await;
        let site_path = temp_dir.path().to_path_buf();

        // Create a static site
        fs::write(site_path.join("index.html"), "Hello World").unwrap();

        Arc::get_mut(&mut site_manager.clone()).unwrap().add_site(
            "parked.test".to_string(),
            site_path.clone(),
            Arc::new(StaticDriver::default()),
        );

        let (client, server_stream) = duplex(1024);
        let mut session = HttpSession::new(Box::new(server_stream));

        // Test index request
        let mut req_header = RequestHeader::build(Method::GET, b"/", None)?;
        req_header.insert_header("Host", "parked.test")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();
    }

    #[tokio::test]
    async fn test_404_request() {
        let (temp_dir, site_manager, server) = setup_test_server().await;
        let site_path = temp_dir.path().to_path_buf();

        // Create a static site
        fs::write(site_path.join("index.html"), "Hello World").unwrap();

        Arc::get_mut(&mut site_manager.clone()).unwrap().add_site(
            "example.com".to_string(),
            site_path.clone(),
            Arc::new(StaticDriver::default()),
        );

        let (client, server_stream) = duplex(1024);
        let mut session = HttpSession::new(Box::new(server_stream));

        // Test nonexistent file request
        let mut req_header = RequestHeader::build(Method::GET, b"/nonexistent.txt", None)?;
        req_header.insert_header("Host", "example.com")?;
        session.req_header = Some(req_header);

        server.handle_request(&mut session).await.unwrap();
    }
}
