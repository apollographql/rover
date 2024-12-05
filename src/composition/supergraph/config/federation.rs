//! Provides objects and utilities to resolve the federation version from user input,
//! including CLI args, [`SupergraphConfig`] input, and subgraph SDLs

use std::marker::PhantomData;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use derive_getters::Getters;

use crate::command::supergraph::compose::do_compose::SupergraphComposeOpts;

use super::full::FullyResolvedSubgraph;

mod state {
    #[derive(Clone, Debug)]
    pub struct FromSupergraphConfig;
    #[derive(Clone, Debug)]
    pub struct FromSubgraphs;
}

/// Error that occurs when the user-selected `FederationVersion` is within Federation 1 boundaries,
/// but the subgraphs use the `@link` directive, which requires Federation 2
#[derive(thiserror::Error, Debug, Getters)]
#[error(
    "The 'federation_version' specified ({}) is invalid. The following subgraphs contain '@link' directives, which are only valid in Federation 2: {}",
    specified_federation_version,
    subgraph_names.join(", ")
)]
pub struct FederationVersionMismatch {
    /// The user specified federation version
    specified_federation_version: FederationVersion,
    /// The subgraph names that have requested Federation 2 features
    subgraph_names: Vec<String>,
}

/// This is the harness for resolving a FederationVersion
#[derive(Clone, Debug)]
pub struct FederationVersionResolver<State: Clone> {
    state: PhantomData<State>,
    federation_version: Option<FederationVersion>,
}

impl Default for FederationVersionResolver<state::FromSupergraphConfig> {
    fn default() -> Self {
        FederationVersionResolver {
            state: PhantomData::<state::FromSupergraphConfig>,
            federation_version: None,
        }
    }
}

/// Represents a `FederationVersionResolver` that has been initiated from user input, if any
/// and is ready to take into account a supergraph config file, resolve immediately, or proceed
/// to a later stage
pub type FederationVersionResolverFromSupergraphConfig =
    FederationVersionResolver<state::FromSupergraphConfig>;

impl FederationVersionResolver<state::FromSupergraphConfig> {
    /// Creates a new `FederationVersionResolver` from a [`FederationVersion`]
    pub fn new(
        federation_version: FederationVersion,
    ) -> FederationVersionResolver<state::FromSupergraphConfig> {
        FederationVersionResolver {
            federation_version: Some(federation_version),
            state: PhantomData::<state::FromSupergraphConfig>,
        }
    }

    /// Produces a new `FederationVersionResolver` that takes into account the [`FederationVersion`]
    /// from a [`SupergraphConfig`] (if it has one)
    pub fn from_supergraph_config(
        self,
        supergraph_config: &SupergraphConfig,
    ) -> FederationVersionResolver<state::FromSubgraphs> {
        let federation_version = self
            .federation_version
            .or(supergraph_config.get_federation_version());
        FederationVersionResolver {
            state: PhantomData::<state::FromSubgraphs>,
            federation_version,
        }
    }

    /// Skips [`SupergraphConfig`] resolution, presumably because there is none
    pub fn skip_supergraph_resolution(self) -> FederationVersionResolver<state::FromSubgraphs> {
        FederationVersionResolver {
            state: PhantomData::<state::FromSubgraphs>,
            federation_version: self.federation_version,
        }
    }

    /// Resolves the federation immediately without taking into account subgraph SDLs
    pub fn resolve(self) -> FederationVersion {
        self.federation_version
            .unwrap_or(FederationVersion::LatestFedTwo)
    }
}

impl From<&SupergraphComposeOpts> for FederationVersionResolver<state::FromSupergraphConfig> {
    fn from(value: &SupergraphComposeOpts) -> Self {
        FederationVersionResolver {
            federation_version: value.federation_version.clone(),
            state: PhantomData::<state::FromSupergraphConfig>,
        }
    }
}

/// Public alias for `FederationVersionResolver<state::FromSubgraphs>`
pub type FederationVersionResolverFromSubgraphs = FederationVersionResolver<state::FromSubgraphs>;

impl FederationVersionResolver<state::FromSubgraphs> {
    #[cfg(test)]
    pub fn new(
        target_federation_version: Option<FederationVersion>,
    ) -> FederationVersionResolver<state::FromSubgraphs> {
        FederationVersionResolver {
            state: PhantomData::<state::FromSubgraphs>,
            federation_version: target_federation_version,
        }
    }

    /// Returns the target [`FederationVersion`] that was defined by the user
    pub fn target_federation_version(&self) -> Option<FederationVersion> {
        self.federation_version.clone()
    }

    /// Resolves the [`FederationVersion`] against user input and the subgraph SDLs provided
    pub fn resolve<'a>(
        &self,
        subgraphs: &'a mut impl Iterator<Item = (&'a String, &'a FullyResolvedSubgraph)>,
    ) -> Result<FederationVersion, FederationVersionMismatch> {
        let fed_two_subgraphs = subgraphs
            .filter_map(|(subgraph_name, subgraph)| {
                if *subgraph.is_fed_two() {
                    Some(subgraph_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let contains_fed_two_subgraphs = !fed_two_subgraphs.is_empty();
        match &self.federation_version {
            Some(specified_federation_version) => {
                let specified_federation_version = specified_federation_version.clone();
                if specified_federation_version.is_fed_one() {
                    if contains_fed_two_subgraphs {
                        Err(FederationVersionMismatch {
                            specified_federation_version,
                            subgraph_names: fed_two_subgraphs,
                        })
                    } else {
                        Ok(specified_federation_version)
                    }
                } else {
                    Ok(specified_federation_version)
                }
            }
            None => Ok(FederationVersion::LatestFedTwo),
        }
    }
}
