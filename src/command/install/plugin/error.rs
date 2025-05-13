use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Invalid plugin version: `{0}`")]
    InvalidVersionFormat(String),
}
