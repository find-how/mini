use std::path::Path;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::driver::{Driver, StaticDriver, LaravelDriver};

/// Registry for site drivers
pub struct DriverRegistry {
    drivers: Vec<Arc<dyn Driver>>,
}

impl DriverRegistry {
    /// Create a new driver registry with default drivers
    pub fn new() -> Self {
        let mut registry = Self {
            drivers: Vec::new(),
        };

        // Register default drivers
        registry.register_driver(Arc::new(StaticDriver));
        registry.register_driver(Arc::new(LaravelDriver));

        registry
    }

    /// Register a new driver
    pub fn register_driver(&mut self, driver: Arc<dyn Driver>) {
        // Add to front of list for higher priority
        self.drivers.insert(0, driver);
    }

    /// Get a driver by name
    pub fn get_driver(&self, name: &str) -> Option<Arc<dyn Driver>> {
        self.drivers.iter()
            .find(|d| d.name() == name)
            .cloned()
    }

    /// Detect the appropriate driver for a site path
    pub fn detect_driver(&self, site_path: &Path) -> Option<Arc<dyn Driver>> {
        self.drivers.iter()
            .find(|d| d.serves(site_path))
            .cloned()
    }

    /// Load a custom driver from a file
    pub fn load_driver_file(&mut self, _path: impl AsRef<Path>) -> Result<()> {
        // TODO: Implement dynamic loading of Rust source files
        // This would require:
        // 1. Parsing the Rust source code
        // 2. Compiling it at runtime
        // 3. Loading the resulting dynamic library
        // For now, we'll return an error
        Err(anyhow!("Dynamic loading of drivers is not yet implemented"))
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use async_trait::async_trait;

    // Mock driver for testing
    struct MockDriver;

    #[async_trait]
    impl Driver for MockDriver {
        fn serves(&self, _site_path: &Path) -> bool {
            true
        }

        fn is_static_file(&self, site_path: &Path, uri: &str) -> Option<String> {
            Some(site_path.join(uri).to_string_lossy().into_owned())
        }

        fn front_controller_path(&self, site_path: &Path) -> String {
            site_path.join("mock.php").to_string_lossy().into_owned()
        }

        fn name(&self) -> &'static str {
            "mock"
        }
    }

    #[test]
    fn test_registry_default_drivers() {
        let registry = DriverRegistry::new();

        // Should have static and Laravel drivers by default
        assert!(registry.get_driver("static").is_some());
        assert!(registry.get_driver("laravel").is_some());

        // Should not have non-existent drivers
        assert!(registry.get_driver("not-found").is_none());
    }

    #[test]
    fn test_registry_register_driver() {
        let mut registry = DriverRegistry::new();
        let mock_driver = Arc::new(MockDriver);

        // Should be able to register a new driver
        registry.register_driver(mock_driver.clone());

        // Should be able to get the registered driver
        let driver = registry.get_driver("mock").unwrap();
        assert_eq!(driver.name(), "mock");
    }

    #[test]
    fn test_registry_detect_driver() {
        let registry = DriverRegistry::new();
        let temp_dir = TempDir::new().unwrap();

        // Should detect static site
        fs::write(temp_dir.path().join("index.html"), "Hello").unwrap();
        let driver = registry.detect_driver(temp_dir.path()).unwrap();
        assert_eq!(driver.name(), "static");

        // Should detect Laravel site
        fs::remove_file(temp_dir.path().join("index.html")).unwrap();
        fs::create_dir_all(temp_dir.path().join("public")).unwrap();
        fs::write(temp_dir.path().join("artisan"), "").unwrap();
        fs::write(temp_dir.path().join("public/index.php"), "").unwrap();
        let driver = registry.detect_driver(temp_dir.path()).unwrap();
        assert_eq!(driver.name(), "laravel");

        // Should return None for unknown site type
        fs::remove_file(temp_dir.path().join("artisan")).unwrap();
        fs::remove_file(temp_dir.path().join("public/index.php")).unwrap();
        assert!(registry.detect_driver(temp_dir.path()).is_none());
    }

    #[test]
    fn test_registry_driver_priority() {
        let mut registry = DriverRegistry::new();
        let mock_driver = Arc::new(MockDriver);

        // Register mock driver that always returns true for serves()
        registry.register_driver(mock_driver);

        let temp_dir = TempDir::new().unwrap();

        // Create both Laravel and static markers
        fs::create_dir_all(temp_dir.path().join("public")).unwrap();
        fs::write(temp_dir.path().join("artisan"), "").unwrap();
        fs::write(temp_dir.path().join("public/index.php"), "").unwrap();
        fs::write(temp_dir.path().join("index.html"), "").unwrap();

        // Mock driver should be detected first as it was registered last
        let driver = registry.detect_driver(temp_dir.path()).unwrap();
        assert_eq!(driver.name(), "mock");
    }

    #[test]
    fn test_registry_load_custom_driver() {
        let mut registry = DriverRegistry::new();
        let temp_dir = TempDir::new().unwrap();

        // Create a custom driver file
        let driver_code = r#"
        use std::path::Path;
        use async_trait::async_trait;
        use anyhow::Result;
        use crate::driver::Driver;

        pub struct CustomDriver;

        #[async_trait]
        impl Driver for CustomDriver {
            fn serves(&self, site_path: &Path) -> bool {
                site_path.join("custom.marker").exists()
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
                site_path.join("custom.php").to_string_lossy().into_owned()
            }

            fn name(&self) -> &'static str {
                "custom"
            }
        }
        "#;

        fs::write(temp_dir.path().join("custom_driver.rs"), driver_code).unwrap();

        // Should return error since dynamic loading is not implemented
        assert!(registry.load_driver_file(temp_dir.path().join("custom_driver.rs")).is_err());
    }
}
