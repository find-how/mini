use std::path::Path;
use async_trait::async_trait;
use anyhow::Result;

/// The Driver trait defines how different types of projects are served
#[async_trait]
pub trait Driver: Send + Sync {
    /// Returns true if this driver can serve the given site path
    fn serves(&self, site_path: &Path) -> bool;

    /// Returns true if the requested URI is a static file
    fn is_static_file(&self, site_path: &Path, uri: &str) -> Option<String>;

    /// Returns the path to the front controller (entry point) for the application
    fn front_controller_path(&self, site_path: &Path) -> String;

    /// Returns the name of the driver
    fn name(&self) -> &'static str;

    /// Optional method to perform any setup when the site is first linked
    async fn setup(&self, _site_path: &Path) -> Result<()> {
        Ok(())
    }
}

/// The StaticDriver serves plain HTML/static sites
pub struct StaticDriver;

#[async_trait]
impl Driver for StaticDriver {
    fn serves(&self, site_path: &Path) -> bool {
        let index_files = ["index.html", "index.htm"];
        index_files.iter().any(|file| site_path.join(file).exists())
    }

    fn is_static_file(&self, site_path: &Path, uri: &str) -> Option<String> {
        let path = site_path.join(uri.trim_start_matches('/'));
        if path.exists() && path.is_file() {
            path.to_str().map(String::from)
        } else {
            None
        }
    }

    fn front_controller_path(&self, site_path: &Path) -> String {
        site_path.join("index.html").to_string_lossy().into_owned()
    }

    fn name(&self) -> &'static str {
        "static"
    }
}

/// The LaravelDriver serves Laravel applications
pub struct LaravelDriver;

#[async_trait]
impl Driver for LaravelDriver {
    fn serves(&self, site_path: &Path) -> bool {
        // Check for typical Laravel markers
        let has_artisan = site_path.join("artisan").exists();
        let has_public_index = site_path.join("public/index.php").exists();
        // We'll use composer.json in the future for version detection
        let _has_composer = site_path.join("composer.json").exists();

        // A Laravel site must have artisan and public/index.php
        has_artisan && has_public_index
    }

    fn is_static_file(&self, site_path: &Path, uri: &str) -> Option<String> {
        let public_path = site_path.join("public");
        let path = public_path.join(uri.trim_start_matches('/'));
        if path.exists() && path.is_file() {
            path.to_str().map(String::from)
        } else {
            None
        }
    }

    fn front_controller_path(&self, site_path: &Path) -> String {
        site_path.join("public/index.php").to_string_lossy().into_owned()
    }

    fn name(&self) -> &'static str {
        "laravel"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Static Driver Tests
    #[test]
    fn test_static_driver_name() {
        let driver = StaticDriver;
        assert_eq!(driver.name(), "static");
    }

    #[test]
    fn test_static_driver_serves() {
        let temp_dir = TempDir::new().unwrap();
        let driver = StaticDriver;

        // Should not serve empty directory
        assert!(!driver.serves(temp_dir.path()));

        // Should not serve directory with random files
        fs::write(temp_dir.path().join("random.txt"), "Hello").unwrap();
        assert!(!driver.serves(temp_dir.path()));

        // Should serve directory with index.html
        fs::write(temp_dir.path().join("index.html"), "Hello").unwrap();
        assert!(driver.serves(temp_dir.path()));

        // Should serve directory with index.htm
        fs::remove_file(temp_dir.path().join("index.html")).unwrap();
        fs::write(temp_dir.path().join("index.htm"), "Hello").unwrap();
        assert!(driver.serves(temp_dir.path()));
    }

    #[test]
    fn test_static_driver_static_files() {
        let temp_dir = TempDir::new().unwrap();
        let driver = StaticDriver;

        // Create some static files
        fs::write(temp_dir.path().join("style.css"), "body {}").unwrap();
        fs::create_dir_all(temp_dir.path().join("assets")).unwrap();
        fs::write(temp_dir.path().join("assets/app.js"), "console.log()").unwrap();

        // Should find existing static files
        assert_eq!(
            driver.is_static_file(temp_dir.path(), "/style.css").unwrap(),
            temp_dir.path().join("style.css").to_string_lossy()
        );
        assert_eq!(
            driver.is_static_file(temp_dir.path(), "/assets/app.js").unwrap(),
            temp_dir.path().join("assets/app.js").to_string_lossy()
        );

        // Should return None for non-existent files
        assert!(driver.is_static_file(temp_dir.path(), "/not-found.css").is_none());
        assert!(driver.is_static_file(temp_dir.path(), "/assets/not-found.js").is_none());

        // Should return None for directories
        assert!(driver.is_static_file(temp_dir.path(), "/assets").is_none());
    }

    #[test]
    fn test_static_driver_front_controller() {
        let temp_dir = TempDir::new().unwrap();
        let driver = StaticDriver;

        assert_eq!(
            driver.front_controller_path(temp_dir.path()),
            temp_dir.path().join("index.html").to_string_lossy()
        );
    }

    // Laravel Driver Tests
    #[test]
    fn test_laravel_driver_name() {
        let driver = LaravelDriver;
        assert_eq!(driver.name(), "laravel");
    }

    #[test]
    fn test_laravel_driver_serves() {
        let temp_dir = TempDir::new().unwrap();
        let driver = LaravelDriver;

        // Should not serve empty directory
        assert!(!driver.serves(temp_dir.path()));

        // Should not serve directory with only artisan
        fs::write(temp_dir.path().join("artisan"), "").unwrap();
        assert!(!driver.serves(temp_dir.path()));

        // Should not serve directory with only public/index.php
        fs::create_dir_all(temp_dir.path().join("public")).unwrap();
        fs::write(temp_dir.path().join("public/index.php"), "").unwrap();
        fs::remove_file(temp_dir.path().join("artisan")).unwrap();
        assert!(!driver.serves(temp_dir.path()));

        // Should serve directory with both artisan and public/index.php
        fs::write(temp_dir.path().join("artisan"), "").unwrap();
        assert!(driver.serves(temp_dir.path()));

        // Should also serve with composer.json
        fs::write(temp_dir.path().join("composer.json"), "{}").unwrap();
        assert!(driver.serves(temp_dir.path()));
    }

    #[test]
    fn test_laravel_driver_static_files() {
        let temp_dir = TempDir::new().unwrap();
        let driver = LaravelDriver;

        // Create public directory and static files
        fs::create_dir_all(temp_dir.path().join("public/assets")).unwrap();
        fs::write(temp_dir.path().join("public/app.css"), "body {}").unwrap();
        fs::write(temp_dir.path().join("public/assets/app.js"), "console.log()").unwrap();

        // Should find existing static files in public directory
        assert_eq!(
            driver.is_static_file(temp_dir.path(), "/app.css").unwrap(),
            temp_dir.path().join("public/app.css").to_string_lossy()
        );
        assert_eq!(
            driver.is_static_file(temp_dir.path(), "/assets/app.js").unwrap(),
            temp_dir.path().join("public/assets/app.js").to_string_lossy()
        );

        // Should return None for non-existent files
        assert!(driver.is_static_file(temp_dir.path(), "/not-found.css").is_none());
        assert!(driver.is_static_file(temp_dir.path(), "/assets/not-found.js").is_none());

        // Should return None for files outside public directory
        fs::write(temp_dir.path().join("secret.txt"), "secret").unwrap();
        assert!(driver.is_static_file(temp_dir.path(), "/secret.txt").is_none());

        // Should return None for directories
        assert!(driver.is_static_file(temp_dir.path(), "/assets").is_none());
    }

    #[test]
    fn test_laravel_driver_front_controller() {
        let temp_dir = TempDir::new().unwrap();
        let driver = LaravelDriver;

        assert_eq!(
            driver.front_controller_path(temp_dir.path()),
            temp_dir.path().join("public/index.php").to_string_lossy()
        );
    }

    #[tokio::test]
    async fn test_laravel_driver_setup() {
        let temp_dir = TempDir::new().unwrap();
        let driver = LaravelDriver;

        // Basic setup should succeed
        assert!(driver.setup(temp_dir.path()).await.is_ok());

        // TODO: Add more setup tests when we implement actual setup logic
    }
}
