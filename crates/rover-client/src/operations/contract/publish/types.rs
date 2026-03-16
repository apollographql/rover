use crate::operations::contract::publish::runner::contract_publish_mutation;

pub(crate) type MutationContractVariantUpsertResult =
    contract_publish_mutation::ContractPublishMutationGraphUpsertContractVariant;
pub(crate) type MutationResponseData = contract_publish_mutation::ResponseData;

type MutationVariables = contract_publish_mutation::Variables;

use rover_studio::types::GraphRef;
use serde::Serialize;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPublishInput {
    pub graph_ref: GraphRef,
    pub source_variant: Option<String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub hide_unreachable_types: bool,
    pub no_launch: bool,
}

impl From<ContractPublishInput> for MutationVariables {
    fn from(input: ContractPublishInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
            source_variant: input.source_variant,
            include: input.include_tags,
            exclude: input.exclude_tags,
            hide_unreachable_types: input.hide_unreachable_types,
            initiate_launch: !input.no_launch,
        }
    }
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug)]
pub struct ContractPublishResponse {
    pub config_description: String,
    pub launch_url: Option<String>,
    pub launch_cli_copy: Option<String>,
}
