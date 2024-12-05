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
    pub health_check_enabled: bool,
    pub health_check_endpoint: Uri,
    #[allow(unused)]
    pub raw_config: String,
}
