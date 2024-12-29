use async_trait::async_trait;
use std::path::{Path, PathBuf};
use anyhow::Result;

#[async_trait]
pub trait Driver: Send + Sync {
    /// Returns the name of the driver
    fn name(&self) -> &'static str;

    /// Checks if this driver can handle the given site path
    fn can_handle(&self, path: &Path) -> bool;

    /// Handles an incoming request
    async fn handle_request(&self, path: &Path, request_path: &str) -> Result<Vec<u8>>;
}

pub struct StaticDriver {
    root_dir: PathBuf,
}

impl StaticDriver {
    pub fn new(root_dir: PathBuf) -> Self {
        StaticDriver { root_dir }
    }

    pub fn default() -> Self {
        StaticDriver {
            root_dir: PathBuf::from("public"),
        }
    }
}

#[async_trait]
impl Driver for StaticDriver {
    fn name(&self) -> &'static str {
        "static"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.join(&self.root_dir).exists()
    }

    async fn handle_request(&self, path: &Path, request_path: &str) -> Result<Vec<u8>> {
        let file_path = path.join(&self.root_dir).join(request_path.trim_start_matches('/'));
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        Ok(tokio::fs::read(file_path).await?)
    }
}

pub struct LaravelDriver {
    root_dir: PathBuf,
    php_version: String,
}

impl LaravelDriver {
    pub fn new(root_dir: PathBuf, php_version: String) -> Self {
        LaravelDriver {
            root_dir,
            php_version,
        }
    }
}

#[async_trait]
impl Driver for LaravelDriver {
    fn name(&self) -> &'static str {
        "laravel"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.join("artisan").exists() && path.join("public/index.php").exists()
    }

    async fn handle_request(&self, path: &Path, request_path: &str) -> Result<Vec<u8>> {
        // In a real implementation, this would execute PHP-FPM with the correct version
        // For now, we'll just return a placeholder response
        let index_php = path.join(&self.root_dir).join("public/index.php");
        if !index_php.exists() {
            return Ok(Vec::new());
        }

        Ok(format!(
            "Laravel site (PHP {}) handling request: {} from {}",
            self.php_version, request_path, index_php.display()
        ).into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_static_driver() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a static site structure
        fs::create_dir_all(root.join("public")).await.unwrap();
        fs::write(
            root.join("public/index.html"),
            "Hello, World!",
        ).await.unwrap();

        let driver = StaticDriver::default();

        // Test detection
        assert!(driver.can_handle(root));

        // Test request handling
        let content = driver.handle_request(root, "/index.html").await.unwrap();
        assert_eq!(String::from_utf8(content).unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_laravel_driver() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a Laravel site structure
        fs::create_dir_all(root.join("public")).await.unwrap();
        fs::write(root.join("artisan"), "").await.unwrap();
        fs::write(root.join("public/index.php"), "").await.unwrap();

        let driver = LaravelDriver::new(root.to_path_buf(), "8.2".to_string());

        // Test detection
        assert!(driver.can_handle(root));

        // Test request handling
        let content = driver.handle_request(root, "/").await.unwrap();
        assert!(String::from_utf8(content).unwrap().contains("Laravel site (PHP 8.2)"));
    }
}
