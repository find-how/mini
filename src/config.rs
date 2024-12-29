use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use tempfile::NamedTempFile;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SiteConfig {
    /// Site root directory
    pub root_dir: String,
    /// Site domain
    pub domain: String,
    /// Whether the site is secured with TLS
    pub secure: bool,
    /// PHP version for this site (if applicable)
    pub php_version: Option<String>,
    /// Environment variables specific to this site
    pub env_vars: HashMap<String, String>,
    /// Custom driver for this site (if any)
    pub driver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    /// Version of the configuration format
    pub version: u32,

    /// Number of worker threads (defaults to number of CPU cores)
    pub threads: usize,

    /// HTTP listen address
    pub http_listen_addr: String,

    /// HTTPS listen address
    pub https_listen_addr: String,

    /// Path to TLS certificate file
    pub tls_cert_path: Option<String>,

    /// Path to TLS key file
    pub tls_key_path: Option<String>,

    /// Path to pid file
    pub pid_file: Option<String>,

    /// Whether to run as daemon
    pub daemon: bool,

    /// Path to error log file
    pub error_log: Option<String>,

    /// User to run as after initialization
    pub user: Option<String>,

    /// Group to run as after initialization
    pub group: Option<String>,

    /// Parked directories (directories containing multiple sites)
    pub parked_paths: Vec<String>,

    /// Linked sites (individual site configurations)
    pub sites: HashMap<String, SiteConfig>,

    /// Default site to serve when no match is found
    pub default_site: Option<String>,

    /// TLD to use for local development (e.g., ".test")
    pub tld: String,

    /// Whether to allow network access from other devices
    pub network_access: bool,

    /// Port for sharing sites (e.g., via ngrok)
    pub share_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            version: 1,
            threads: num_cpus::get(),
            http_listen_addr: "127.0.0.1:80".to_string(),
            https_listen_addr: "127.0.0.1:443".to_string(),
            tls_cert_path: None,
            tls_key_path: None,
            pid_file: None,
            daemon: false,
            error_log: None,
            user: None,
            group: None,
            parked_paths: Vec::new(),
            sites: HashMap::new(),
            default_site: None,
            tld: ".test".to_string(),
            network_access: false,
            share_port: 8080,
        }
    }
}

impl ServerConfig {
    /// Load configuration from YAML file
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn to_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }

    /// Add a parked directory
    pub fn add_parked_path<S: Into<String>>(&mut self, path: S) {
        self.parked_paths.push(path.into());
    }

    /// Remove a parked directory
    pub fn remove_parked_path<S: AsRef<str>>(&mut self, path: S) {
        self.parked_paths.retain(|p| p != path.as_ref());
    }

    /// Add or update a site configuration
    pub fn add_site(&mut self, domain: String, config: SiteConfig) {
        self.sites.insert(domain, config);
    }

    /// Remove a site configuration
    pub fn remove_site<S: AsRef<str>>(&mut self, domain: S) -> Option<SiteConfig> {
        self.sites.remove(domain.as_ref())
    }

    /// Set the default site
    pub fn set_default_site<S: Into<String>>(&mut self, path: Option<S>) {
        self.default_site = path.map(|p| p.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.threads, num_cpus::get());
        assert_eq!(config.http_listen_addr, "127.0.0.1:80");
        assert_eq!(config.https_listen_addr, "127.0.0.1:443");
        assert_eq!(config.tls_cert_path, None);
        assert_eq!(config.tls_key_path, None);
        assert_eq!(config.pid_file, None);
        assert!(!config.daemon);
        assert_eq!(config.error_log, None);
        assert_eq!(config.user, None);
        assert_eq!(config.group, None);
        assert!(config.parked_paths.is_empty());
        assert!(config.sites.is_empty());
        assert_eq!(config.default_site, None);
        assert_eq!(config.tld, ".test");
        assert!(!config.network_access);
        assert_eq!(config.share_port, 8080);
    }

    #[test]
    fn test_site_management() {
        let mut config = ServerConfig::default();

        // Test parking paths
        config.add_parked_path("/Users/test/Sites");
        assert_eq!(config.parked_paths.len(), 1);
        config.remove_parked_path("/Users/test/Sites");
        assert!(config.parked_paths.is_empty());

        // Test site configuration
        let site_config = SiteConfig {
            root_dir: "/Users/test/Sites/myapp".to_string(),
            domain: "myapp.test".to_string(),
            secure: true,
            php_version: Some("8.2".to_string()),
            env_vars: {
                let mut map = HashMap::new();
                map.insert("APP_ENV".to_string(), "local".to_string());
                map
            },
            driver: Some("laravel".to_string()),
        };

        config.add_site("myapp.test".to_string(), site_config.clone());
        assert_eq!(config.sites.len(), 1);

        let retrieved = config.sites.get("myapp.test").unwrap();
        assert_eq!(retrieved.domain, "myapp.test");
        assert_eq!(retrieved.php_version, Some("8.2".to_string()));

        config.remove_site("myapp.test");
        assert!(config.sites.is_empty());
    }

    #[test]
    fn test_default_site() {
        let mut config = ServerConfig::default();

        config.set_default_site(Some("/Users/test/Sites/default"));
        assert_eq!(config.default_site, Some("/Users/test/Sites/default".to_string()));

        config.set_default_site(None::<String>);
        assert_eq!(config.default_site, None);
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut config = ServerConfig::default();
        config.version = 2;
        config.add_parked_path("/Users/test/Sites");

        let site_config = SiteConfig {
            root_dir: "/Users/test/Sites/myapp".to_string(),
            domain: "myapp.test".to_string(),
            secure: true,
            php_version: Some("8.2".to_string()),
            env_vars: {
                let mut map = HashMap::new();
                map.insert("APP_ENV".to_string(), "local".to_string());
                map
            },
            driver: Some("laravel".to_string()),
        };

        config.add_site("myapp.test".to_string(), site_config);
        config.set_default_site(Some("/Users/test/Sites/default"));

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: ServerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_load_save_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = ServerConfig::default();
        config.version = 2;
        config.add_parked_path("/Users/test/Sites");

        let site_config = SiteConfig {
            root_dir: "/Users/test/Sites/myapp".to_string(),
            domain: "myapp.test".to_string(),
            secure: true,
            php_version: Some("8.2".to_string()),
            env_vars: {
                let mut map = HashMap::new();
                map.insert("APP_ENV".to_string(), "local".to_string());
                map
            },
            driver: Some("laravel".to_string()),
        };

        config.add_site("myapp.test".to_string(), site_config);
        config.set_default_site(Some("/Users/test/Sites/default"));

        let temp_file = NamedTempFile::new()?;
        config.to_yaml(&temp_file)?;

        let loaded = ServerConfig::from_yaml(&temp_file)?;
        assert_eq!(config, loaded);

        Ok(())
    }
}
