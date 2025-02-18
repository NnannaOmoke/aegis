use crate::store::InMemoryStore;

use std::net::Ipv4Addr;

use serde::Deserialize;
use thiserror::Error;

type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct AegisConfig {
    address: Ipv4Addr,
    backend_config: Vec<BackendConfig>,
    route_config: Vec<RouteConfig>,
}

impl AegisConfig {
    fn validate_config(&self) -> ConfigResult<()> {
        Ok(())
    }

    pub fn to_store(&self) -> InMemoryStore {
        let bcap = self.backend_config.len();
        let rcap = self.route_config.len();
        let mut store = InMemoryStore::init_empty(rcap, bcap);
        store.fill(self);
        store
    }

    pub fn backend_config(&self) -> &Vec<BackendConfig> {
        &self.backend_config
    }

    pub fn route_config(&self) -> &Vec<RouteConfig> {
        &self.route_config
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct BackendConfig {
    name: Option<String>,
    pub prefix: Option<String>,
    pub url: String,
    pub rate_limit_ip_min: Option<u32>,
    pub rate_limit_token_min: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RouteConfig {
    name: Option<String>,
    pub url: String,
    pub rate_limit_ip_min: Option<u32>,
    pub rate_limit_token_min: Option<u32>,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Validating the TOML file failed: {}", .0)]
    ValidationError(String),
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_serialize_config() {
        let parsed: AegisConfig = toml::from_str(
            r#"
        address = '127.0.0.1'
        
        [[backend_config]]
        name = 'apache-one'
        prefix = '/api/'
        url = 'http://localhost:3000'
        rate_limit_ip_min = 10
        rate_limit_token_min = 20
        
        [[backend_config]]
        name = 'apache-two'
        prefix = '/internal/'
        url = 'http://localhost:4000'
        rate_limit_ip_min = 10
        rate_limit_token_min = 20
        
        [[route_config]]
        name = 'route-one'
        url = 'http://localhost:300/route'
        rate_limit_ip_min = 50
        rate_limit_token_min = 100
        "#,
        )
        .unwrap();
        assert_eq!(parsed.address, Ipv4Addr::from_str("127.0.0.1").unwrap());
        assert_eq!(
            parsed.backend_config,
            vec![
                BackendConfig {
                    name: Some(String::from("apache-one")),
                    prefix: Some(String::from("/api/")),
                    url: String::from("http://localhost:3000"),
                    rate_limit_ip_min: Some(10),
                    rate_limit_token_min: Some(20),
                },
                BackendConfig {
                    name: Some(String::from("apache-two")),
                    prefix: Some(String::from("/internal/")),
                    url: String::from("http://localhost:4000"),
                    rate_limit_ip_min: Some(10),
                    rate_limit_token_min: Some(20)
                },
            ]
        );
        assert_eq!(
            parsed.route_config,
            vec![RouteConfig {
                name: Some(String::from("route-one")),
                url: String::from("http://localhost:300/route"),
                rate_limit_ip_min: Some(50),
                rate_limit_token_min: Some(100),
            }]
        );
    }
}
