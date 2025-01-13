use std::{
    io::Error,
    net::{SocketAddr, SocketAddrV4, ToSocketAddrs},
    str::FromStr,
};

use thiserror::Error;

use super::RouterAddress;

#[derive(Error, Debug)]
pub enum ParseRouterConfigError {
    #[error("Invalid SocketAddr at {}. Error: {:?}", .path, .source)]
    ParseAddress {
        path: &'static str,
        source: std::io::Error,
    },
}

pub struct RouterConfigParser<'a> {
    yaml: &'a serde_yaml::Value,
    address: SocketAddr,
}

impl<'a> RouterConfigParser<'a> {
    pub fn new(yaml: &'a serde_yaml::Value, address: SocketAddr) -> RouterConfigParser<'a> {
        RouterConfigParser { yaml, address }
    }
    pub fn address(&self) -> Result<SocketAddr, ParseRouterConfigError> {
        let config_address = self
            .yaml
            .get("supergraph")
            .and_then(|s| s.get("listen"))
            .and_then(|l| l.as_str())
            .and_then(|s| {
                let s = if s.contains(":") {
                    s.to_string()
                } else {
                    format!("{}:80", s)
                };
                s.to_socket_addrs()
                    .map(|mut addrs| addrs.next())
                    .transpose()
            })
            .transpose()
            .map_err(|err| ParseRouterConfigError::ParseAddress {
                path: "supergraph.listen",
                source: err,
            })?;

        let default_address: SocketAddr = RouterAddress::default().into();
        // Resolution precendence for addresses:
        // 1) CLI option
        // 2) Config
        // 4) Environment variable
        // 3) Default
        //
        // `self.address` gets set by first looking at the environment variable and then the CLI
        // option; otherwise, we set it to the default
        //
        // If `self.address` doesn't match the default, we have either an environment variable or
        // CLI option (and we rely on the proper handling of that elsewhere)
        if self.address != default_address {
            // So, send it back!
            Ok(self.address)
        } else if config_address.is_some() {
            // Otherwise, if we have a config address, send it back
            Ok(config_address.unwrap_or(default_address))
        } else {
            // Lastly, if no env var or config address are found, return the default
            Ok(default_address)
        }
    }
    pub fn health_check_enabled(&self) -> bool {
        self.yaml
            .get("health_check")
            .or_else(|| self.yaml.get("health-check"))
            .and_then(|health_check| health_check.as_mapping())
            .and_then(|health_check| health_check.get("enabled"))
            .and_then(|enabled| enabled.as_bool())
            .unwrap_or_default()
    }
    pub fn listen_path(&self) -> Option<String> {
        self.yaml
            .get("supergraph")
            .and_then(|supergraph| supergraph.as_mapping())
            .and_then(|supergraph| supergraph.get("path"))
            .and_then(|path| path.as_str().map(|path| path.to_string()))
    }
    /// Gets the user-defined health_check_endpoint or, if missing, returns the default
    /// 127.0.0.1:8088
    ///
    /// See https://www.apollographql.com/docs/graphos/routing/self-hosted/health-checks
    pub fn health_check_endpoint(&self) -> Result<Option<SocketAddr>, ParseRouterConfigError> {
        Ok(Some(
            self.yaml
                .get("health_check")
                .or_else(|| self.yaml.get("health-check"))
                .and_then(|health_check| health_check.as_mapping())
                .and_then(|health_check| health_check.get("listen"))
                .and_then(|addr_and_port| addr_and_port.as_str())
                .and_then(|path| {
                    path.to_string()
                        .to_socket_addrs()
                        .map(|mut addrs| addrs.next())
                        .transpose()
                })
                .transpose()
                .map_err(|err| ParseRouterConfigError::ParseAddress {
                    path: "health_check.listen",
                    source: err,
                })?
                .unwrap_or(SocketAddr::V4(
                    SocketAddrV4::from_str("127.0.0.1:8088").map_err(|err| {
                        ParseRouterConfigError::ParseAddress {
                            path: "health_check.listen",
                            source: Error::new(std::io::ErrorKind::InvalidInput, err.to_string()),
                        }
                    })?,
                )),
        ))
    }
    /// Gets the user-defined health_check_path or, if absent, returns the default `/health`
    ///
    /// See https://www.apollographql.com/docs/graphos/routing/self-hosted/health-checks
    pub fn health_check_path(&self) -> String {
        self.yaml
            .get("health_check")
            .or_else(|| self.yaml.get("health-check"))
            .and_then(|health_check| health_check.as_mapping())
            .and_then(|health_check| health_check.get("path"))
            .and_then(|path| path.as_str())
            .unwrap_or("/health")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, str::FromStr};

    use anyhow::Result;
    use rstest::rstest;
    use speculoos::prelude::*;

    use crate::command::dev::next::router::config::RouterAddress;

    use super::RouterConfigParser;

    #[rstest]
    #[case("127.0.0.1", SocketAddr::from_str("127.0.0.1:80").unwrap())]
    #[case("127.0.0.1:8000", SocketAddr::from_str("127.0.0.1:8000").unwrap())]
    #[case("localhost", SocketAddr::from_str("[::1]:80").unwrap())]
    #[case("localhost:8000", SocketAddr::from_str("[::1]:8000").unwrap())]
    fn test_get_address_from_router_config(
        #[case] socket_addr_str: &str,
        #[case] expected_socket_addr: SocketAddr,
    ) -> Result<()> {
        let config_yaml_str = format!(
            indoc::indoc! {
                r#"---
supergraph:
  listen: {}
"#
            },
            socket_addr_str
        );
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: expected_socket_addr,
        };
        let address = router_config.address();
        assert_that!(address)
            .is_ok()
            .is_equal_to(expected_socket_addr);
        Ok(())
    }

    #[rstest]
    #[case::cli_override_over_config(SocketAddr::from_str("127.0.0.1:8089").unwrap(), "127.0.0.1:4000", SocketAddr::from_str("127.0.0.1:8089").unwrap())]
    // When the default is passed in, we use the config's address
    #[case::config_over_default(Into::<SocketAddr>::into(RouterAddress::default()), "127.0.0.1:8089", SocketAddr::from_str("127.0.0.1:8089").unwrap())]
    fn test_get_address_from_router_config_with_override(
        #[case] cli_override_addr: SocketAddr,
        #[case] config_addr: &str,
        #[case] expected_socket_addr: SocketAddr,
    ) -> Result<()> {
        let config_yaml_str = format!(
            indoc::indoc! {
                r#"---
supergraph:
  listen: {}
"#
            },
            config_addr
        );
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: cli_override_addr,
        };
        let address = router_config.address();
        assert_that!(address)
            .is_ok()
            .is_equal_to(expected_socket_addr);
        Ok(())
    }

    #[rstest]
    fn test_get_health_check(#[values(true, false)] is_health_check_enabled: bool) -> Result<()> {
        let config_yaml_str = format!(
            indoc::indoc! {
                r#"---
health_check:
  enabled: {}
"#
            },
            is_health_check_enabled
        );
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: SocketAddr::from_str("127.0.0.1:80")?,
        };
        let health_check_enabled = router_config.health_check_enabled();
        assert_that!(health_check_enabled).is_equal_to(is_health_check_enabled);
        Ok(())
    }

    #[rstest]
    fn test_get_health_default() -> Result<()> {
        let config_yaml_str = indoc::indoc! {
            r#"---
        "#
        };
        let config_yaml = serde_yaml::from_str(config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: SocketAddr::from_str("127.0.0.1:80")?,
        };
        let health_check = router_config.health_check_endpoint()?.unwrap().to_string();

        assert_that!(health_check).is_equal_to("127.0.0.1:8088".to_string());
        Ok(())
    }

    #[rstest]
    fn test_get_listen_path_default() -> Result<()> {
        let config_yaml_str = indoc::indoc! {
            r#"---
        "#
        };
        let config_yaml = serde_yaml::from_str(config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: SocketAddr::from_str("127.0.0.1:80")?,
        };
        assert_that!(router_config.listen_path()).is_none();
        Ok(())
    }

    #[rstest]
    fn test_get_listen_path() -> Result<()> {
        let config_yaml_str = indoc::indoc! {
            r#"---
supergraph:
  path: /custom-path
"#
        };
        let config_yaml = serde_yaml::from_str(config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,

            address: SocketAddr::from_str("127.0.0.1:80")?,
        };
        assert_that!(router_config.listen_path())
            .is_some()
            .is_equal_to("/custom-path".to_string());
        Ok(())
    }
}
