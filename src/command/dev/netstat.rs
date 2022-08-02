use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags};
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use reqwest::{blocking::Client, Url};

use super::introspect::IntrospectRunner;

pub fn get_all_local_endpoints() -> Vec<Url> {
    get_all_local_endpoints_except(&Vec::new())
}

pub fn get_all_local_endpoints_except(excluded: &Vec<Url>) -> Vec<Url> {
    let mut local_endpoints = Vec::new();
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    if let Ok(sockets_info) = get_sockets_info(af_flags, proto_flags) {
        for si in &sockets_info {
            if &si.local_addr().to_string() == "::" {
                let url = format!("http://localhost:{}", si.local_port())
                    .parse()
                    .unwrap();
                if !excluded.contains(&url) {
                    local_endpoints.push(url);
                }
            }
        }
    }
    local_endpoints
}

pub fn get_all_local_graphql_endpoints(client: Client) -> Vec<Url> {
    get_all_local_graphql_endpoints_except(client, &Vec::new())
}

pub fn get_all_local_graphql_endpoints_except(client: Client, excluded: &Vec<Url>) -> Vec<Url> {
    get_all_local_endpoints_except(excluded)
        .par_iter()
        .filter_map(|endpoint| {
            let introspect_runner = IntrospectRunner::new(endpoint.clone(), client.clone());
            if introspect_runner.run().is_ok() {
                Some(endpoint.clone())
            } else {
                None
            }
        })
        .collect()
}
