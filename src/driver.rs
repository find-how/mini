use std::path::PathBuf;
use anyhow::Result;

pub struct LaravelDriver {
    // TODO: Add fields when needed
}

impl LaravelDriver {
    pub fn new(path: PathBuf, php_version: String) -> Self {
        LaravelDriver {}
    }

    pub fn start(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laravel_driver() {
        let driver = LaravelDriver::new(PathBuf::from("/path/to/app"), "8.1".to_string());
        assert!(driver.start().is_ok());
    }
}
