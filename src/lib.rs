pub mod driver;
pub mod registry;
pub mod site;

pub use driver::{Driver, LaravelDriver, StaticDriver};
pub use registry::DriverRegistry;
pub use site::{Site, SiteManager};

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{Result, anyhow};

// Global state
lazy_static::lazy_static! {
    static ref SITE_MANAGER: Arc<SiteManager> = Arc::new(SiteManager::new());
}

/// Initialize a new Valet-like development environment
pub async fn init() -> Result<()> {
    // TODO: Initialize the development environment
    Ok(())
}

/// Park a directory for auto-discovery of sites
pub async fn park(path: PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Directory does not exist: {}", path.display()));
    }

    SITE_MANAGER.park_path(path)
}

/// Unpark a directory
pub async fn unpark(path: PathBuf) -> Result<()> {
    SITE_MANAGER.unpark_path(&path)
}

/// Link a site with a specific domain
pub async fn link(domain: String, path: PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Site directory does not exist: {}", path.display()));
    }

    let site = Site {
        name: domain.clone(),
        path,
        secure: false,
        php_version: None,
    };
    SITE_MANAGER.add_site(&domain, site)
}

/// Unlink a site
pub async fn unlink(domain: String) -> Result<()> {
    SITE_MANAGER.remove_site(&domain)
}

/// Secure a site with TLS
pub async fn secure(domain: String) -> Result<()> {
    SITE_MANAGER.secure_site(&domain)
}

/// Remove TLS from a site
pub async fn unsecure(domain: String) -> Result<()> {
    SITE_MANAGER.unsecure_site(&domain)
}

/// Set PHP version for a site
pub async fn isolate(domain: String, version: String) -> Result<()> {
    SITE_MANAGER.set_php_version(&domain, version)
}

/// Remove PHP version isolation from a site
pub async fn unisolate(domain: String) -> Result<()> {
    SITE_MANAGER.set_php_version(&domain, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn setup_test_env() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        (temp_dir, root)
    }

    #[tokio::test]
    async fn test_park_unpark() -> Result<()> {
        let (_temp_dir, root) = setup_test_env().await;
        let sites_dir = root.join("sites");
        fs::create_dir_all(&sites_dir).await?;

        // Test parking
        park(sites_dir.clone()).await?;

        // Test unparking
        unpark(sites_dir).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_link_unlink() -> Result<()> {
        let (_temp_dir, root) = setup_test_env().await;
        let site_dir = root.join("mysite");
        fs::create_dir_all(&site_dir).await?;

        // Test linking
        link("mysite.test".to_string(), site_dir.clone()).await?;

        // Test unlinking
        unlink("mysite.test".to_string()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_secure_unsecure() -> Result<()> {
        let (_temp_dir, root) = setup_test_env().await;
        let site_dir = root.join("mysite");
        fs::create_dir_all(&site_dir).await?;

        // Setup site first
        link("mysite.test".to_string(), site_dir).await?;

        // Test securing
        secure("mysite.test".to_string()).await?;

        // Test unsecuring
        unsecure("mysite.test".to_string()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_php_isolation() -> Result<()> {
        let (_temp_dir, root) = setup_test_env().await;
        let site_dir = root.join("mysite");
        fs::create_dir_all(&site_dir).await?;

        // Setup site first
        link("mysite.test".to_string(), site_dir).await?;

        // Test PHP version isolation
        isolate("mysite.test".to_string(), "8.2".to_string()).await?;

        // Test PHP version unisolation
        unisolate("mysite.test".to_string()).await?;

        Ok(())
    }
}
