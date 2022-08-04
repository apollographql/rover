use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use reqwest::{blocking::Client, Url};

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
            let url = if &si.local_addr().to_string() == "::" {
                format!("http://localhost:{}", si.local_port())
            } else {
                format!("http://{}:{}", si.local_addr(), si.local_port())
            };
            if let Ok(url) = url.parse() {
                if !excluded.contains(&url) {
                    local_endpoints.push(url);
                }
            }
        }
    }
    local_endpoints
}

pub fn get_all_local_graphql_endpoints_except(client: Client, excluded: &[Url]) -> Vec<Url> {
    get_all_local_endpoints_except(excluded)
        .par_iter()
        .filter_map(|endpoint| {
            let introspect_runner = UnknownIntrospectRunner::new(endpoint.clone(), client.clone());
            if introspect_runner.run().is_ok() {
                Some(endpoint.clone())
            } else {
                None
            }
        })
        .collect()
}
