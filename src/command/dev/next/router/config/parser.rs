use std::{
    net::{SocketAddr, ToSocketAddrs},
    str::FromStr,
};

use http::{uri::InvalidUri, Uri};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseRouterConfigError {
    #[error("Invalid SocketAddr at {}. Error: {:?}", .path, .source)]
    ParseAddress {
        path: &'static str,
        source: std::io::Error,
    },
    #[error("Invalid Uri at {}. Error: {:?}", .path, .source)]
    ListenPath {
        path: &'static str,
        source: InvalidUri,
    },
    #[error("Invalid Uri when combining the health_check address, port, and path. Error: {:?}", .source)]
    HealthCheckEndpoint { source: InvalidUri },
}

pub struct RouterConfigParser<'a> {
    yaml: &'a serde_yaml::Value,
}

impl<'a> RouterConfigParser<'a> {
    pub fn new(yaml: &'a serde_yaml::Value) -> RouterConfigParser<'a> {
        RouterConfigParser { yaml }
    }
    pub fn address(&self) -> Result<Option<SocketAddr>, ParseRouterConfigError> {
        self.yaml
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
            })
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
    pub fn listen_path(&self) -> Result<Option<Uri>, ParseRouterConfigError> {
        self.yaml
            .get("supergraph")
            .and_then(|supergraph| supergraph.as_mapping())
            .and_then(|supergraph| supergraph.get("path"))
            .and_then(|path| path.as_str())
            .and_then(|path| Some(Uri::from_str(path)))
            .transpose()
            .map_err(|err| ParseRouterConfigError::ListenPath {
                path: "supergraph.path",
                source: err,
            })
    }
    pub fn health_check_endpoint(&self) -> Result<Uri, ParseRouterConfigError> {
        let addr_and_port = self
            .yaml
            .get("health_check")
            .or_else(|| self.yaml.get("health-check"))
            .and_then(|health_check| health_check.as_mapping())
            .and_then(|health_check| health_check.get("listen"))
            .and_then(|addr_and_port| addr_and_port.as_str())
            .and_then(|addr_and_port| Some(Uri::from_str(addr_and_port)))
            // See https://www.apollographql.com/docs/graphos/routing/self-hosted/health-checks for
            // defaults
            .unwrap_or(Uri::from_str("127.0.0.1:8088"))
            .map_err(|err| ParseRouterConfigError::ListenPath {
                path: "health_check.listen",
                source: err,
            })?;

        let path = self
            .yaml
            .get("health_check")
            .or_else(|| self.yaml.get("health-check"))
            .and_then(|health_check| health_check.as_mapping())
            .and_then(|health_check| health_check.get("path"))
            .and_then(|path| path.as_str())
            // See https://www.apollographql.com/docs/graphos/routing/self-hosted/health-checks for
            // defaults
            .unwrap_or("/health");

        let mut health_check_endpoint = addr_and_port.to_string();
        health_check_endpoint.push_str(path);

        Ok(Uri::from_str(&health_check_endpoint)
            .map_err(|err| ParseRouterConfigError::HealthCheckEndpoint { source: err })?)
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, str::FromStr};

    use anyhow::Result;
    use http::Uri;
    use rstest::rstest;
    use speculoos::prelude::*;

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
        let router_config = RouterConfigParser { yaml: &config_yaml };
        let address = router_config.address();
        assert_that!(address)
            .is_ok()
            .is_some()
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
        let router_config = RouterConfigParser { yaml: &config_yaml };
        let health_check = router_config.health_check();
        assert_that!(health_check).is_equal_to(is_health_check_enabled);
        Ok(())
    }

    #[rstest]
    fn test_get_health_default() -> Result<()> {
        let config_yaml_str = indoc::indoc! {
            r#"---
        "#
        };
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser { yaml: &config_yaml };
        let health_check = router_config.health_check();
        assert_that!(health_check).is_false();
        Ok(())
    }

    #[rstest]
    fn test_get_listen_path_default() -> Result<()> {
        let config_yaml_str = indoc::indoc! {
            r#"---
        "#
        };
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser { yaml: &config_yaml };
        let health_check = router_config.listen_path();
        assert_that!(health_check).is_ok().is_none();
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
        let config_yaml = serde_yaml::from_str(&config_yaml_str)?;
        let router_config = RouterConfigParser { yaml: &config_yaml };
        let address = router_config.listen_path();
        assert_that!(address)
            .is_ok()
            .is_some()
            .is_equal_to(Uri::from_str("/custom-path").unwrap());
        Ok(())
    }
}
