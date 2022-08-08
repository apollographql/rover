use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};
use rayon::{iter::IntoParallelRefIterator, prelude::ParallelIterator};
use reqwest::{blocking::Client, Url};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
use url::Host;

use super::introspect::UnknownIntrospectRunner;

pub fn get_all_local_endpoints_except(excluded: &[SocketAddr]) -> Vec<SocketAddr> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    Vec::from_iter(
        if let Ok(sockets_info) = get_sockets_info(af_flags, proto_flags) {
            sockets_info
                .par_iter()
                .filter_map(|si| {
                    let socket_addr = SocketAddr::from((si.local_addr(), si.local_port()));
                    if !excluded.contains(&socket_addr) {
                        Some(socket_addr)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<SocketAddr>>()
        } else {
            HashSet::new()
        },
    )
}

pub fn get_all_local_graphql_endpoints_except(client: Client, excluded: &[SocketAddr]) -> Vec<Url> {
    let get_graphql_endpoint = |client: Client, endpoint: SocketAddr| -> Option<Url> {
        let try_get = |runner: &UnknownIntrospectRunner, endpoint: &Url| -> Option<Url> {
            tracing::info!("attempting to introspect {}", endpoint);
            if runner.run().is_ok() {
                Some(endpoint.clone())
            } else {
                None
            }
        };
        if let Ok(mut url) = format!("http://{}", endpoint).parse::<Url>() {
            let runner = UnknownIntrospectRunner::new(url.clone(), client);
            try_get(&runner, &url).or_else(|| {
                url.set_path("graphql");
                try_get(&runner, &url).or_else(|| {
                    url.set_path("query");
                    try_get(&runner, &url)
                })
            })
        } else {
            None
        }
    };

    Vec::from_iter(
        get_all_local_endpoints_except(excluded)
            .par_iter()
            .filter_map(|endpoint| get_graphql_endpoint(client.clone(), endpoint.clone()))
            .collect::<HashSet<Url>>(),
    )
}

pub fn normalize_loopback_urls(url: &Url) -> Vec<Url> {
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
                    Vec::new()
                }
            }
        },
        None => Vec::new(),
    };
    Vec::from_iter(
        hosts
            .iter()
            .filter_map(|host| {
                let mut url = url.clone();
                let _ = url.set_host(Some(&host));
                Some(url)
            })
            .collect::<HashSet<Url>>(),
    )
}
