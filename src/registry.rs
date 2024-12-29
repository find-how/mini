use std::path::PathBuf;
use std::sync::Arc;
use crate::driver::Driver;

pub struct DriverRegistry {
    drivers: Vec<Arc<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry {
            drivers: Vec::new(),
        }
    }

    pub fn register(&mut self, driver: Arc<dyn Driver>) {
        self.drivers.push(driver);
    }

    pub fn get_driver(&self, path: &PathBuf) -> Option<&Arc<dyn Driver>> {
        self.drivers.iter().find(|driver| driver.supports(path))
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

        // Create Laravel site structure
        let laravel_site = root.join("laravel-site");
        fs::create_dir_all(laravel_site.join("public")).await.unwrap();
        fs::write(laravel_site.join("artisan"), "").await.unwrap();
        fs::write(laravel_site.join("public/index.php"), "").await.unwrap();

        // Create static site structure
        let static_site = root.join("static-site");
        fs::create_dir_all(static_site.join("public")).await.unwrap();
        fs::write(static_site.join("public/index.html"), "").await.unwrap();

        let mut registry = DriverRegistry::new();

        // Register Laravel driver
        let laravel_driver = Arc::new(LaravelDriver::new(
            laravel_site.clone(),
            "8.2".to_string(),
        ));
        registry.register(laravel_driver);

        // Test driver detection
        let detected_driver = registry.get_driver(&laravel_site);
        assert!(detected_driver.is_some());
        assert_eq!(detected_driver.unwrap().name(), "Laravel");

        // Test no driver for static site
        let detected_driver = registry.get_driver(&static_site);
        assert!(detected_driver.is_none());
    }
}
