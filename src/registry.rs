use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::driver::Driver;

pub struct DriverRegistry {
    drivers: RwLock<HashMap<String, Arc<dyn Driver>>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry {
            drivers: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, driver: Arc<dyn Driver>) {
        let mut drivers = self.drivers.write().unwrap();
        drivers.insert(driver.name().to_string(), driver);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Driver>> {
        let drivers = self.drivers.read().unwrap();
        drivers.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::driver::LaravelDriver;

    #[tokio::test]
    async fn test_driver_registry() {
        let registry = DriverRegistry::new();
        let driver = Arc::new(LaravelDriver::new(
            PathBuf::from("/path/to/app"),
            "8.2".to_string(),
        ));

        registry.register(driver.clone());
        let retrieved = registry.get("Laravel").unwrap();
        assert_eq!(retrieved.name(), "Laravel");
    }
}
