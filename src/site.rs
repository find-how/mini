use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;

use crate::registry::DriverRegistry;

/// Represents a site configuration.
/// Currently only used for testing, but will be expanded in the future
/// to support more site-specific configuration.
#[derive(Clone)]
pub struct Site {
    domain: String,
    path: PathBuf,
    secure: bool,
}

impl Site {
    pub fn new(domain: String, path: PathBuf) -> Self {
        Site {
            domain,
            path,
            secure: false,
        }
    }

    pub fn secure(&mut self) {
        self.secure = true;
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn is_secure(&self) -> bool {
        self.secure
    }
}

/// Manages site configurations and their associated drivers.
/// The registry field is currently unused but will be used in the future
/// to manage site drivers.
pub struct SiteManager {
    sites: Arc<RwLock<HashMap<String, Site>>>,
    registry: Arc<DriverRegistry>,
}

impl SiteManager {
    pub fn new(registry: Arc<DriverRegistry>) -> Self {
        SiteManager {
            sites: Arc::new(RwLock::new(HashMap::new())),
            registry,
        }
    }

    pub async fn add_site(&self, domain: &str, path: PathBuf) -> Result<()> {
        let mut sites = self.sites.write().await;
        sites.insert(domain.to_string(), Site::new(domain.to_string(), path));
        Ok(())
    }

    pub async fn secure_site(&self, domain: &str) -> Result<()> {
        let mut sites = self.sites.write().await;
        if let Some(site) = sites.get_mut(domain) {
            site.secure();
            Ok(())
        } else {
            anyhow::bail!("Site not found")
        }
    }

    pub async fn get_site(&self, domain: &str) -> Option<Site> {
        let sites = self.sites.read().await;
        sites.get(domain).cloned()
    }

    pub async fn start_site(&self, domain: &str) -> Result<()> {
        let sites = self.sites.read().await;
        if let Some(site) = sites.get(domain) {
            // Try to find a driver that supports this site
            if let Some(driver) = self.registry.get("Laravel") {
                if driver.supports(site.path()) {
                    return driver.start().await;
                }
            }
            anyhow::bail!("No suitable driver found for site")
        } else {
            anyhow::bail!("Site not found")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;
    use crate::driver::LaravelDriver;

    #[tokio::test]
    async fn test_site_manager() {
        let registry = Arc::new(DriverRegistry::new());
        let manager = SiteManager::new(registry.clone());

        let temp_dir = TempDir::new().unwrap();
        let site_path = temp_dir.path().to_path_buf();

        // Create Laravel site structure
        fs::create_dir_all(site_path.join("public")).await.unwrap();
        fs::write(site_path.join("artisan"), "").await.unwrap();
        fs::write(site_path.join("public/index.php"), "").await.unwrap();

        // Register Laravel driver
        registry.register(Arc::new(LaravelDriver::new(
            site_path.clone(),
            "8.2".to_string(),
        )));

        // Test adding a site
        manager.add_site("example.test", site_path.clone()).await.unwrap();

        // Test getting a site
        let site = manager.get_site("example.test").await.unwrap();
        assert_eq!(site.domain(), "example.test");
        assert_eq!(site.path(), &site_path);
        assert!(!site.is_secure());

        // Test securing a site
        manager.secure_site("example.test").await.unwrap();
        let site = manager.get_site("example.test").await.unwrap();
        assert!(site.is_secure());

        // Test starting a site
        assert!(manager.start_site("example.test").await.is_ok());

        // Test getting a non-existent site
        assert!(manager.get_site("nonexistent.test").await.is_none());
    }
}
