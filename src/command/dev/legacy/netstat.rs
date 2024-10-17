use reqwest::Url;
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};
use url::Host;

use crate::command::dev::legacy::protocol::SubgraphUrl;

pub fn normalize_loopback_urls(url: &SubgraphUrl) -> Vec<SubgraphUrl> {
    let hosts = match url.host() {
        Some(host) => match host {
            Host::Ipv4(ip) => {
                if &ip.to_string() == "::" {
                    vec![
                        IpAddr::V4(ip).to_string(),
                        IpAddr::V4(Ipv4Addr::LOCALHOST).to_string(),
                    ]
                } else {
                    vec![IpAddr::V4(ip).to_string()]
                }
            }
            Host::Ipv6(ip) => {
                if &ip.to_string() == "::" || &ip.to_string() == "::1" {
                    vec![
                        IpAddr::V6(ip).to_string(),
                        IpAddr::V6(Ipv6Addr::LOCALHOST).to_string(),
                    ]
                } else {
                    vec![IpAddr::V6(ip).to_string()]
                }
            }
            Host::Domain(domain) => {
                if domain == "localhost" {
                    vec![
                        IpAddr::V4(Ipv4Addr::LOCALHOST).to_string(),
                        IpAddr::V6(Ipv6Addr::LOCALHOST).to_string(),
                        "[::]".to_string(),
                        "0.0.0.0".to_string(),
                    ]
                } else {
                    vec![domain.to_string()]
                }
            }
        },
        None => Vec::new(),
    };
    if hosts.is_empty() {
        vec![url.clone()]
    } else {
        Vec::from_iter(
            hosts
                .iter()
                .map(|host| {
                    let mut url = url.clone();
                    let _ = url.set_host(Some(host));
                    url
                })
                .collect::<HashSet<Url>>(),
        )
    }
}
