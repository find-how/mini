use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use anyhow::{anyhow, Result};

#[derive(Clone, Debug)]
pub struct Site {
    pub path: PathBuf,
    pub secure: bool,
    pub php_version: Option<String>,
}

impl Site {
    pub fn new(path: PathBuf) -> Self {
        Site {
            path,
            secure: false,
            php_version: None,
        }
    }
}

pub struct SiteManager {
    sites: RwLock<HashMap<String, Site>>,
    paths: RwLock<Vec<PathBuf>>,
}

impl SiteManager {
    pub fn new() -> Self {
        SiteManager {
            sites: RwLock::new(HashMap::new()),
            paths: RwLock::new(Vec::new()),
        }
    }

    pub fn park_path(&self, path: PathBuf) -> Result<()> {
        let mut paths = self.paths.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        if !paths.contains(&path) {
            paths.push(path);
        }
        Ok(())
    }

    pub fn unpark_path(&self, path: &PathBuf) -> Result<()> {
        let mut paths = self.paths.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        if let Some(pos) = paths.iter().position(|p| p == path) {
            paths.remove(pos);
            Ok(())
        } else {
            Err(anyhow!("Path not found"))
        }
    }

    pub fn add_site(&self, domain: &str, site: Site) -> Result<()> {
        let mut sites = self.sites.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        sites.insert(domain.to_string(), site);
        Ok(())
    }

    pub fn remove_site(&self, domain: &str) -> Result<()> {
        let mut sites = self.sites.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        sites.remove(domain);
        Ok(())
    }

    pub fn get_site(&self, domain: &str) -> Option<Site> {
        self.sites.read().ok()?.get(domain).cloned()
    }

    pub fn secure_site(&self, domain: &str) -> Result<()> {
        let mut sites = self.sites.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        if let Some(site) = sites.get_mut(domain) {
            site.secure = true;
            Ok(())
        } else {
            Err(anyhow!("Site not found"))
        }
    }

    pub fn unsecure_site(&self, domain: &str) -> Result<()> {
        let mut sites = self.sites.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        if let Some(site) = sites.get_mut(domain) {
            site.secure = false;
            Ok(())
        } else {
            Err(anyhow!("Site not found"))
        }
    }

    pub fn set_php_version(&self, domain: &str, version: String) -> Result<()> {
        let mut sites = self.sites.write().map_err(|_| anyhow!("Failed to acquire write lock"))?;
        if let Some(site) = sites.get_mut(domain) {
            site.php_version = if version.is_empty() { None } else { Some(version) };
            Ok(())
        } else {
            Err(anyhow!("Site not found"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_site_manager() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test site paths
        let site1_path = root.join("site1");
        let site2_path = root.join("site2");
        fs::create_dir_all(&site1_path).await.unwrap();
        fs::create_dir_all(&site2_path).await.unwrap();

        let manager = SiteManager::new();

        // Test parking paths
        manager.park_path(site1_path.clone()).unwrap();
        manager.park_path(site2_path.clone()).unwrap();

        // Test adding sites
        let site1 = Site::new(site1_path.clone());
        let site2 = Site::new(site2_path.clone());
        manager.add_site("site1.test", site1).unwrap();
        manager.add_site("site2.test", site2).unwrap();

        // Test getting sites
        let retrieved_site1 = manager.get_site("site1.test").unwrap();
        assert_eq!(retrieved_site1.path, site1_path);
        assert!(!retrieved_site1.secure);
        assert_eq!(retrieved_site1.php_version, None);

        // Test securing site
        manager.secure_site("site1.test").unwrap();
        let secured_site = manager.get_site("site1.test").unwrap();
        assert!(secured_site.secure);

        // Test setting PHP version
        manager.set_php_version("site1.test", "8.2".to_string()).unwrap();
        let php_site = manager.get_site("site1.test").unwrap();
        assert_eq!(php_site.php_version, Some("8.2".to_string()));

        // Test clearing PHP version
        manager.set_php_version("site1.test", String::new()).unwrap();
        let cleared_site = manager.get_site("site1.test").unwrap();
        assert_eq!(cleared_site.php_version, None);

        // Test removing sites
        manager.remove_site("site1.test").unwrap();
        assert!(manager.get_site("site1.test").is_none());

        // Test unparking paths
        manager.unpark_path(&site1_path).unwrap();
        assert!(manager.unpark_path(&site1_path).is_err());
    }
}
