use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};
use rayon::{iter::IntoParallelRefIterator, prelude::ParallelIterator};
use reqwest::{blocking::Client, Url};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
use url::Host;

use crate::command::dev::socket::SubgraphUrl;

use super::introspect::UnknownIntrospectRunner;

pub fn get_all_local_sockets_except(excluded_socket_addrs: &[SocketAddr]) -> Vec<SocketAddr> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    let sockets = if let Ok(sockets_info) = get_sockets_info(af_flags, proto_flags) {
        HashMap::from_iter(sockets_info.iter().filter_map(|si| {
            let local_addr = si.local_addr();
            let local_port = si.local_port();
            if excluded_socket_addrs
                .par_iter()
                .find_any(|s| s.port() == local_port)
                .is_some()
            {
                None
            } else {
                Some((local_port, local_addr))
            }
        }))
    } else {
        HashMap::new()
    };

    Vec::from_iter(
        sockets
            .iter()
            .map(|(port, addr)| SocketAddr::from((*addr, *port))),
    )
}

pub fn get_all_local_graphql_endpoints_except(
    client: Client,
    excluded_socket_addrs: &[SocketAddr],
) -> Vec<SubgraphUrl> {
    let get_graphql_endpoint = |client: Client, socket_addr: SocketAddr| -> Option<Url> {
        let try_get = |runner: &UnknownIntrospectRunner, endpoint: &Url| -> Option<Url> {
            tracing::info!("attempting to introspect {}", endpoint);
            if runner.run().is_ok() {
                Some(endpoint.clone())
            } else {
                None
            }
        };
        if let Ok(mut url) = format!("http://{}", socket_addr).parse::<Url>() {
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

    let local_sockets = get_all_local_sockets_except(excluded_socket_addrs);

    Vec::from_iter(
        local_sockets
            .par_iter()
            .filter_map(|socket_addr| get_graphql_endpoint(client.clone(), *socket_addr))
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
            .map(|host| {
                let mut url = url.clone();
                let _ = url.set_host(Some(host));
                url
            })
            .collect::<HashSet<Url>>(),
    )
}
