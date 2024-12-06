use http::Uri;

use super::RouterAddress;

pub struct RunRouterConfigDefault;

pub struct RunRouterConfigReadConfig {
    pub router_address: RouterAddress,
}

#[derive(Default)]
pub struct RunRouterConfigFinal {
    #[allow(unused)]
    pub listen_path: Option<Uri>,
    #[allow(unused)]
    pub address: RouterAddress,
    #[allow(unused)]
    pub health_check: bool,
    #[allow(unused)]
    pub raw_config: String,
}
