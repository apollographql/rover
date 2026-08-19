use url::Url;

use super::{DeviceAuthorizationFlowClient, StandardDeviceAuthorizationResponse};

#[derive(Debug)]
pub struct DeviceAuthorizationFlowInit {
    pub client_id: String,
    pub device_authorization_url: Url,
    pub token_url: Url,
}

#[derive(Debug)]
pub struct DeviceAuthorizationFlowWithDeviceCode {
    pub client: DeviceAuthorizationFlowClient,
    pub device_auth_response: StandardDeviceAuthorizationResponse,
}
