use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Site {
    pub name: String,
    pub path: PathBuf,
    pub secure: bool,
    pub php_version: Option<String>,
}

pub struct SiteManager {
    sites: RwLock<HashMap<String, Site>>,
    parked_paths: RwLock<Vec<PathBuf>>,
}

impl SiteManager {
    pub fn new() -> Self {
        SiteManager {
            sites: RwLock::new(HashMap::new()),
            parked_paths: RwLock::new(Vec::new()),
        }
    }

    pub fn add_site(&self, name: &str, site: Site) -> Result<()> {
        let mut sites = self.sites.write().unwrap();
        sites.insert(name.to_string(), site);
        Ok(())
    }

    pub fn get_site(&self, name: &str) -> Option<Site> {
        let sites = self.sites.read().unwrap();
        sites.get(name).cloned()
    }

    pub fn remove_site(&self, name: &str) -> Result<()> {
        let mut sites = self.sites.write().unwrap();
        sites.remove(name);
        Ok(())
    }

    pub fn park_path(&self, path: PathBuf) -> Result<()> {
        let mut paths = self.parked_paths.write().unwrap();
        if !paths.contains(&path) {
            paths.push(path);
        }
        Ok(())
    }

    pub fn unpark_path(&self, path: &Path) -> Result<()> {
        let mut paths = self.parked_paths.write().unwrap();
        paths.retain(|p| p != path);
        Ok(())
    }

    pub fn get_parked_paths(&self) -> Vec<PathBuf> {
        let paths = self.parked_paths.read().unwrap();
        paths.clone()
    }

    pub fn secure_site(&self, name: &str) -> Result<()> {
        let mut sites = self.sites.write().unwrap();
        if let Some(site) = sites.get_mut(name) {
            site.secure = true;
        }
        Ok(())
    }

    pub fn unsecure_site(&self, name: &str) -> Result<()> {
        let mut sites = self.sites.write().unwrap();
        if let Some(site) = sites.get_mut(name) {
            site.secure = false;
        }
        Ok(())
    }

    pub fn set_php_version(&self, name: &str, version: String) -> Result<()> {
        let mut sites = self.sites.write().unwrap();
        if let Some(site) = sites.get_mut(name) {
            site.php_version = Some(version);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_site_management() {
        let manager = SiteManager::new();
        let temp_dir = TempDir::new().unwrap();
        let site_path = temp_dir.path().to_path_buf();

        let site = Site {
            name: "test.test".to_string(),
            path: site_path.clone(),
            secure: false,
            php_version: None,
        };

        // Test adding a site
        manager.add_site("test.test", site.clone()).unwrap();
        assert_eq!(manager.get_site("test.test").unwrap().name, "test.test");

        // Test securing a site
        manager.secure_site("test.test").unwrap();
        assert!(manager.get_site("test.test").unwrap().secure);

        // Test unsecuring a site
        manager.unsecure_site("test.test").unwrap();
        assert!(!manager.get_site("test.test").unwrap().secure);

        // Test setting PHP version
        manager.set_php_version("test.test", "8.2".to_string()).unwrap();
        assert_eq!(manager.get_site("test.test").unwrap().php_version.unwrap(), "8.2");

        // Test removing a site
        manager.remove_site("test.test").unwrap();
        assert!(manager.get_site("test.test").is_none());
    }

    #[test]
    fn test_parked_paths() {
        let manager = SiteManager::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Test parking a path
        manager.park_path(path.clone()).unwrap();
        assert!(manager.get_parked_paths().contains(&path));

        // Test unparking a path
        manager.unpark_path(&path).unwrap();
        assert!(!manager.get_parked_paths().contains(&path));
    }
}
