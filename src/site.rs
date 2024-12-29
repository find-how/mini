use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use crate::driver::Driver;
use crate::registry::DriverRegistry;
use tokio::fs;

/// Represents a site configuration
#[derive(Clone)]
pub struct Site {
    /// Domain name (e.g., "example.test")
    pub domain: String,
    /// Root directory path
    pub root_dir: PathBuf,
    /// Whether the site is secured with TLS
    pub secure: bool,
    /// PHP version for this site (if applicable)
    pub php_version: Option<String>,
    /// The driver used to serve this site
    pub driver: Arc<dyn Driver>,
}

/// Manages site registration and configuration
pub struct SiteManager {
    /// Registry for site drivers
    driver_registry: DriverRegistry,
    /// Parked directories
    parked_dirs: HashSet<PathBuf>,
    /// Linked sites by domain
    sites: HashMap<String, Site>,
}

impl SiteManager {
    /// Create a new site manager
    pub fn new(driver_registry: DriverRegistry) -> Self {
        Self {
            driver_registry,
            parked_dirs: HashSet::new(),
            sites: HashMap::new(),
        }
    }

    /// Park a directory for auto-detection of sites
    pub async fn park_directory(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(anyhow!("Directory does not exist: {}", path.display()));
        }
        if self.parked_dirs.contains(&path) {
            return Err(anyhow!("Directory is already parked: {}", path.display()));
        }
        self.parked_dirs.insert(path);
        Ok(())
    }

    /// Unpark a directory
    pub async fn unpark_directory(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        if !self.parked_dirs.remove(&path) {
            return Err(anyhow!("Directory is not parked: {}", path.display()));
        }
        Ok(())
    }

    /// Check if a directory is parked
    pub fn is_parked(&self, path: impl AsRef<Path>) -> bool {
        self.parked_dirs.contains(path.as_ref())
    }

    /// Link a site with a specific domain
    pub async fn link_site(&mut self, domain: impl Into<String>, path: impl AsRef<Path>) -> Result<()> {
        let domain = domain.into();
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            return Err(anyhow!("Site directory does not exist: {}", path.display()));
        }
        if self.sites.contains_key(&domain) {
            return Err(anyhow!("Domain is already linked: {}", domain));
        }

        let driver = self.driver_registry.detect_driver(&path)
            .ok_or_else(|| anyhow!("No suitable driver found for site: {}", path.display()))?;

        let site = Site {
            domain: domain.clone(),
            root_dir: path,
            secure: false,
            php_version: None,
            driver,
        };

        self.sites.insert(domain, site);
        Ok(())
    }

    /// Unlink a site
    pub async fn unlink_site(&mut self, domain: impl AsRef<str>) -> Result<()> {
        let domain = domain.as_ref();
        if !self.sites.remove(domain).is_some() {
            return Err(anyhow!("Site is not linked: {}", domain));
        }
        Ok(())
    }

    /// Check if a domain is linked
    pub fn is_linked(&self, domain: impl AsRef<str>) -> bool {
        self.sites.contains_key(domain.as_ref())
    }

    /// Secure a site with TLS
    pub async fn secure_site(&mut self, domain: impl AsRef<str>) -> Result<()> {
        let domain = domain.as_ref();
        let site = self.sites.get_mut(domain)
            .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

        // TODO: Generate TLS certificate
        site.secure = true;
        Ok(())
    }

    /// Remove TLS from a site
    pub async fn unsecure_site(&mut self, domain: impl AsRef<str>) -> Result<()> {
        let domain = domain.as_ref();
        let site = self.sites.get_mut(domain)
            .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

        // TODO: Remove TLS certificate
        site.secure = false;
        Ok(())
    }

    /// Check if a site is secured with TLS
    pub fn is_secure(&self, domain: impl AsRef<str>) -> bool {
        self.sites.get(domain.as_ref())
            .map(|site| site.secure)
            .unwrap_or(false)
    }

    /// Scan a parked directory for sites
    pub async fn scan_parked_directory(&mut self, path: impl AsRef<Path>) -> Result<Vec<String>> {
        let path = path.as_ref();
        let mut linked_domains = Vec::new();

        let mut entries = fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let site_path = entry.path();
                if let Some(_) = self.driver_registry.detect_driver(&site_path) {
                    let domain = format!("{}.test", entry.file_name().to_string_lossy());
                    if !self.is_linked(&domain) {
                        self.link_site(&domain, &site_path).await?;
                        linked_domains.push(domain);
                    }
                }
            }
        }

        Ok(linked_domains)
    }

    /// Get site information by domain
    pub fn get_site(&self, domain: impl AsRef<str>) -> Option<&Site> {
        self.sites.get(domain.as_ref())
    }

    /// Set PHP version for a site
    pub async fn isolate_php(&mut self, domain: impl AsRef<str>, version: impl Into<String>) -> Result<()> {
        let domain = domain.as_ref();
        let site = self.sites.get_mut(domain)
            .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

        site.php_version = Some(version.into());
        Ok(())
    }

    /// Remove PHP version isolation from a site
    pub async fn unisolate_php(&mut self, domain: impl AsRef<str>) -> Result<()> {
        let domain = domain.as_ref();
        let site = self.sites.get_mut(domain)
            .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

        site.php_version = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_site_manager_park() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a directory to park
        let sites_dir = temp_dir.path().join("sites");
        fs::create_dir(&sites_dir).unwrap();

        // Should be able to park a directory
        manager.park_directory(&sites_dir).await.unwrap();
        assert!(manager.is_parked(&sites_dir));

        // Should not be able to park the same directory twice
        assert!(manager.park_directory(&sites_dir).await.is_err());

        // Should be able to unpark a directory
        manager.unpark_directory(&sites_dir).await.unwrap();
        assert!(!manager.is_parked(&sites_dir));
    }

    #[tokio::test]
    async fn test_site_manager_link() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a static site
        let site_dir = temp_dir.path().join("mysite");
        fs::create_dir(&site_dir).unwrap();
        fs::write(site_dir.join("index.html"), "Hello").unwrap();

        // Should be able to link a site
        manager.link_site("mysite.test", &site_dir).await.unwrap();
        assert!(manager.is_linked("mysite.test"));

        // Should not be able to link the same domain twice
        assert!(manager.link_site("mysite.test", &site_dir).await.is_err());

        // Should be able to unlink a site
        manager.unlink_site("mysite.test").await.unwrap();
        assert!(!manager.is_linked("mysite.test"));
    }

    #[tokio::test]
    async fn test_site_manager_secure() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a static site
        let site_dir = temp_dir.path().join("mysite");
        fs::create_dir(&site_dir).unwrap();
        fs::write(site_dir.join("index.html"), "Hello").unwrap();

        // Link the site first
        manager.link_site("mysite.test", &site_dir).await.unwrap();

        // Should be able to secure a site
        manager.secure_site("mysite.test").await.unwrap();
        assert!(manager.is_secure("mysite.test"));

        // Should be able to unsecure a site
        manager.unsecure_site("mysite.test").await.unwrap();
        assert!(!manager.is_secure("mysite.test"));

        // Should fail to secure non-existent site
        assert!(manager.secure_site("nonexistent.test").await.is_err());
    }

    #[tokio::test]
    async fn test_site_manager_scan_parked() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a parked directory with multiple sites
        let sites_dir = temp_dir.path().join("sites");
        fs::create_dir(&sites_dir).unwrap();

        // Create site1 (static)
        let site1_dir = sites_dir.join("site1");
        fs::create_dir(&site1_dir).unwrap();
        fs::write(site1_dir.join("index.html"), "Hello").unwrap();

        // Create site2 (static)
        let site2_dir = sites_dir.join("site2");
        fs::create_dir(&site2_dir).unwrap();
        fs::write(site2_dir.join("index.html"), "Hello").unwrap();

        // Park the directory
        manager.park_directory(&sites_dir).await.unwrap();

        // Should auto-detect and link sites in parked directory
        let sites = manager.scan_parked_directory(&sites_dir).await.unwrap();
        assert_eq!(sites.len(), 2);
        assert!(manager.is_linked("site1.test"));
        assert!(manager.is_linked("site2.test"));
    }

    #[tokio::test]
    async fn test_site_manager_get_site() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a static site
        let site_dir = temp_dir.path().join("mysite");
        fs::create_dir(&site_dir).unwrap();
        fs::write(site_dir.join("index.html"), "Hello").unwrap();

        // Link the site
        manager.link_site("mysite.test", &site_dir).await.unwrap();

        // Should be able to get site info
        let site = manager.get_site("mysite.test").unwrap();
        assert_eq!(site.domain, "mysite.test");
        assert_eq!(site.root_dir, site_dir);
        assert!(!site.secure);
        assert_eq!(site.driver.name(), "static");

        // Should return None for non-existent site
        assert!(manager.get_site("nonexistent.test").is_none());
    }

    #[tokio::test]
    async fn test_site_manager_isolate_php() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SiteManager::new(DriverRegistry::new());

        // Create a Laravel site
        let site_dir = temp_dir.path().join("mysite");
        fs::create_dir_all(site_dir.join("public")).unwrap();
        fs::write(site_dir.join("artisan"), "").unwrap();
        fs::write(site_dir.join("public/index.php"), "").unwrap();

        // Link the site
        manager.link_site("mysite.test", &site_dir).await.unwrap();

        // Should be able to set PHP version
        manager.isolate_php("mysite.test", "8.2").await.unwrap();
        let site = manager.get_site("mysite.test").unwrap();
        assert_eq!(site.php_version, Some("8.2".to_string()));

        // Should be able to remove PHP isolation
        manager.unisolate_php("mysite.test").await.unwrap();
        let site = manager.get_site("mysite.test").unwrap();
        assert_eq!(site.php_version, None);

        // Should fail for non-existent site
        assert!(manager.isolate_php("nonexistent.test", "8.2").await.is_err());
    }
}
