use std::path::Path;
use std::sync::Arc;
use crate::driver::{Driver, StaticDriver};

pub struct DriverRegistry {
    drivers: Vec<Arc<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        let mut registry = DriverRegistry {
            drivers: Vec::new(),
        };

        // Register default drivers (static driver is the fallback)
        registry.register(Arc::new(StaticDriver::default()));
        registry
    }

    pub fn register(&mut self, driver: Arc<dyn Driver>) {
        // Insert more specific drivers at the beginning
        self.drivers.insert(0, driver);
    }

    pub fn get_driver(&self, path: &Path) -> Option<Arc<dyn Driver>> {
        self.drivers
            .iter()
            .find(|driver| driver.can_handle(path))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::LaravelDriver;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_driver_registry() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test site structures
        let static_site = root.join("static");
        let laravel_site = root.join("laravel");

        // Setup static site
        fs::create_dir_all(static_site.join("public")).await.unwrap();
        fs::write(
            static_site.join("public/index.html"),
            "Hello, World!",
        ).await.unwrap();

        // Setup Laravel site
        fs::create_dir_all(laravel_site.join("public")).await.unwrap();
        fs::write(laravel_site.join("artisan"), "").await.unwrap();
        fs::write(laravel_site.join("public/index.php"), "").await.unwrap();

        let mut registry = DriverRegistry::new();

        // Register Laravel driver
        registry.register(Arc::new(LaravelDriver::new(
            laravel_site.clone(),
            "8.2".to_string(),
        )));

        // Test Laravel site detection first (since it's more specific)
        let driver = registry.get_driver(&laravel_site).unwrap();
        assert_eq!(driver.name(), "laravel");

        // Test static site detection
        let driver = registry.get_driver(&static_site).unwrap();
        assert_eq!(driver.name(), "static");

        // Test unknown site
        let unknown_site = root.join("unknown");
        fs::create_dir(&unknown_site).await.unwrap();
        assert!(registry.get_driver(&unknown_site).is_none());
    }
}
