use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};
use rayon::{iter::IntoParallelRefMutIterator, prelude::ParallelIterator};
use reqwest::{blocking::Client, Url};
use std::{net::SocketAddr, str::FromStr};

use super::introspect::UnknownIntrospectRunner;

pub fn get_all_local_endpoints() -> Vec<Url> {
    get_all_local_endpoints_except(&Vec::new())
}

pub fn get_all_local_endpoints_except(excluded: &[Url]) -> Vec<Url> {
    let mut local_endpoints = Vec::new();
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    if let Ok(sockets_info) = get_sockets_info(af_flags, proto_flags) {
        for si in &sockets_info {
            let socket_addr = SocketAddr::from((si.local_addr(), si.local_port()));
            if let Ok(url) = Url::from_str(format!("http://{}", &socket_addr).as_str()) {
                if !(excluded.contains(&url) || local_endpoints.contains(&url)) {
                    local_endpoints.push(url);
                }
            }
        }
    }
    local_endpoints
}

pub fn get_all_local_graphql_endpoints_except(client: Client, excluded: &[Url]) -> Vec<Url> {
    let get_endpoint = |client: Client, endpoint: Url| -> Option<Url> {
        tracing::info!("attempting to introspect {}", &endpoint);
        let introspect_runner = UnknownIntrospectRunner::new(endpoint.clone(), client);
        if introspect_runner.run().is_ok() {
            Some(endpoint.clone())
        } else {
            None
        }
    };

    get_all_local_endpoints_except(excluded)
        .par_iter_mut()
        .filter_map(|endpoint| {
            get_endpoint(client.clone(), endpoint.clone()).or_else(|| {
                endpoint.set_path("graphql");
                get_endpoint(client.clone(), endpoint.clone()).or_else(|| {
                    endpoint.set_path("query");
                    get_endpoint(client.clone(), endpoint.clone())
                })
            })
        })
        .collect()
}
