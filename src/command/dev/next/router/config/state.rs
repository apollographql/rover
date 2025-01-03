use std::net::SocketAddr;

use super::RouterAddress;

pub struct RunRouterConfigDefault;

pub struct RunRouterConfigReadConfig {
    pub router_address: RouterAddress,
}

pub struct RunRouterConfigFinal {
    #[allow(unused)]
    pub listen_path: Option<String>,
    #[allow(unused)]
    pub address: RouterAddress,
    pub health_check_enabled: bool,
    pub health_check_endpoint: Option<SocketAddr>,
    pub health_check_path: String,
    #[allow(unused)]
    pub raw_config: String,
}

impl Default for RunRouterConfigFinal {
    fn default() -> Self {
        Self {
            listen_path: Option::default(),
            address: RouterAddress::default(),
            health_check_enabled: bool::default(),
            health_check_endpoint: Option::default(),
            health_check_path: "/health".to_string(),
            raw_config: String::default(),
        }
    }
}
