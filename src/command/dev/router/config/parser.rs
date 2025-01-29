use std::io::Error;
use std::net::{SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::str::FromStr;

use thiserror::Error;

use super::{RouterAddress, RouterHost, RouterPort};

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
    address: RouterAddress,
}

impl<'a> RouterConfigParser<'a> {
    pub fn new(yaml: &'a serde_yaml::Value, address: RouterAddress) -> RouterConfigParser<'a> {
        RouterConfigParser { yaml, address }
    }
    pub fn address(&self) -> Result<RouterAddress, ParseRouterConfigError> {
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

        // Resolution precendence for addresses and ports:
        // 1) CLI option
        // 2) Config
        // 3) Default
        let port = match (config_address, self.address.port) {
            (Some(_), RouterPort::CliOption(port)) => RouterPort::CliOption(port),
            (Some(addr), RouterPort::ConfigFile(..)) | (Some(addr), RouterPort::Default(..)) => {
                RouterPort::ConfigFile(addr.port())
            }
            (None, port) => port,
        };

        let host = match (config_address, self.address.host) {
            (Some(_), RouterHost::CliOption(addr)) => RouterHost::CliOption(addr),
            (Some(addr), RouterHost::ConfigFile(..)) | (Some(addr), RouterHost::Default(..)) => {
                RouterHost::ConfigFile(addr.ip())
            }
            (None, host) => host,
        };

        Ok(RouterAddress::new(Some(host), Some(port)))
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
    use anyhow::Result;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::RouterConfigParser;
    use crate::command::dev::router::config::{
        RouterAddress, RouterHost, RouterPort, DEFAULT_ROUTER_IP_ADDR, DEFAULT_ROUTER_PORT,
    };

    #[rstest]
    #[case("127.0.0.1", RouterAddress::new(Some(RouterHost::ConfigFile("127.0.0.1".parse()?)), Some(RouterPort::ConfigFile(80))))]
    #[case("127.0.0.1:8000", RouterAddress::new(Some(RouterHost::ConfigFile("127.0.0.1".parse()?)), Some(RouterPort::ConfigFile(8000))))]
    #[case("localhost", RouterAddress::new(Some(RouterHost::ConfigFile("::1".parse()?)), Some(RouterPort::ConfigFile(80))))]
    #[case("localhost:8000", RouterAddress::new(Some(RouterHost::ConfigFile("::1".parse()?)), Some(RouterPort::ConfigFile(8000))))]
    fn test_get_address_from_router_config(
        #[case] socket_addr_str: &str,
        #[case] expected_router_address: RouterAddress,
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
            address: expected_router_address,
        };
        let address = router_config.address();
        assert_that!(address)
            .is_ok()
            .is_equal_to(expected_router_address);
        Ok(())
    }

    #[rstest]
    #[case::no_overrides_gives_default(
        None,
        None,
        None,
        RouterAddress::new(Some(DEFAULT_ROUTER_IP_ADDR), Some(DEFAULT_ROUTER_PORT))
    )]
    #[case::cli_host_overrides_default(
        Some(RouterHost::CliOption("129.0.0.1".parse()?)),
        None,
        None,
        RouterAddress::new(Some(RouterHost::CliOption("129.0.0.1".parse()?)), Some(DEFAULT_ROUTER_PORT))
    )]
    #[case::cli_port_overrides_default(
        None,
        Some(RouterPort::CliOption(9999)),
        None,
        RouterAddress::new(Some(DEFAULT_ROUTER_IP_ADDR), Some(RouterPort::CliOption(9999)))
    )]
    #[case::cli_host_and_port_overrides_default(
        Some(RouterHost::CliOption("129.0.0.1".parse()?)),
        Some(RouterPort::CliOption(9999)),
        None,
        RouterAddress::new(Some(RouterHost::CliOption("129.0.0.1".parse()?)), Some(RouterPort::CliOption(9999)))
    )]
    #[case::cli_host_and_port_overrides_even_config(
        Some(RouterHost::CliOption("129.0.0.1".parse()?)),
        Some(RouterPort::CliOption(9999)),
        Some("127.0.0.1:1234"),
        RouterAddress::new(Some(RouterHost::CliOption("129.0.0.1".parse()?)), Some(RouterPort::CliOption(9999)))
    )]
    #[case::config_overrides_default_but_only_for_address(
        None,
        Some(RouterPort::CliOption(9999)),
        Some("127.0.0.1:1234"),
        RouterAddress::new(Some(RouterHost::ConfigFile("127.0.0.1".parse()?)), Some(RouterPort::CliOption(9999)))
    )]
    #[case::config_overrides_default_but_only_for_port(
        Some(RouterHost::CliOption("129.0.0.1".parse()?)),
        None,
        Some("127.0.0.1:1234"),
        RouterAddress::new(Some(RouterHost::CliOption("129.0.0.1".parse()?)), Some(RouterPort::ConfigFile(1234)))
    )]
    #[case::config_overrides_default_no_cli_options(
        None,
        None,
        Some("127.0.0.1:1234"),
        RouterAddress::new(Some(RouterHost::ConfigFile("127.0.0.1".parse()?)), Some(RouterPort::ConfigFile(1234)))
    )]
    fn test_get_address_from_router_config_with_override(
        #[case] cli_override_host: Option<RouterHost>,
        #[case] cli_override_port: Option<RouterPort>,
        #[case] config_addr: Option<&str>,
        #[case] expected_router_address: RouterAddress,
    ) -> Result<()> {
        let config_yaml_str = match config_addr {
            Some(config_addr) => {
                format!(
                    indoc::indoc! {
                        r#"---
supergraph:
  listen: {}
telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
"#
                    },
                    config_addr
                )
            }
            None => String::from(indoc::indoc! {
                r#"---
telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
"#
            }),
        };
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser {
            yaml: &config_yaml,
            address: RouterAddress::new(cli_override_host, cli_override_port),
        };
        let address = router_config.address();
        assert_that!(address)
            .is_ok()
            .is_equal_to(expected_router_address);
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
            address: RouterAddress::new(None, None),
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
            address: RouterAddress::new(None, None),
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
            address: RouterAddress::new(None, None),
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

            address: RouterAddress::new(None, None),
        };
        assert_that!(router_config.listen_path())
            .is_some()
            .is_equal_to("/custom-path".to_string());
        Ok(())
    }
}
