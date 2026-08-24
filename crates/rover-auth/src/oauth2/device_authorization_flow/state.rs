use url::Url;

use super::{DeviceAuthorizationFlowClient, StandardDeviceAuthorizationResponse};

pub trait DeviceAuthorizationFlowState {}

#[derive(Debug)]
pub struct DeviceAuthorizationFlowInit {
    pub client_id: String,
    pub device_authorization_url: Url,
    pub token_url: Url,
}

impl DeviceAuthorizationFlowState for DeviceAuthorizationFlowInit {}

#[derive(Debug)]
pub struct DeviceAuthorizationFlowWithDeviceCode {
    pub client: DeviceAuthorizationFlowClient,
    pub device_auth_response: StandardDeviceAuthorizationResponse,
}

impl DeviceAuthorizationFlowState for DeviceAuthorizationFlowWithDeviceCode {}
