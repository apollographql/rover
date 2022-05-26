use crate::operations::graph::async_check::runner::graph_async_check_mutation;
use crate::shared::{CheckConfig, GitContext, GraphRef};

type MutationInput = graph_async_check_mutation::CheckSchemaAsyncInput;
type MutationConfig = graph_async_check_mutation::HistoricQueryParametersInput;
type MutationGitContextInput = graph_async_check_mutation::GitContextInput;
type MutationVariables = graph_async_check_mutation::Variables;
pub(crate) type MutationResponseData = graph_async_check_mutation::ResponseData;

#[derive(Debug, Clone, PartialEq)]
pub struct CheckSchemaAsyncInput {
  pub graph_ref: GraphRef,
  pub proposed_schema: String,
  pub git_context: GitContext,
  pub config: CheckConfig,
}

impl From<CheckSchemaAsyncInput> for MutationVariables {
  fn from(input: CheckSchemaAsyncInput) -> Self {
    let graph_ref = input.graph_ref.clone();
    Self {
      graph_id: input.graph_ref.name,
      name: input.graph_ref.variant,
      input: MutationInput {
        graphRef: graph_ref.to_string(),
        proposedSchemaDocument: Some(input.proposed_schema),
        gitContext: input.git_context.into(),
        config: input.config.into(),
        isSandbox: false,
        introspectionEndpoint: Some("todo remove this".to_string()), // can't pass none for now because of bug in api
      }
    }
  }
}

impl From<CheckConfig> for MutationConfig {
  fn from(input: CheckConfig) -> Self {
      let (from, to) = match input.validation_period {
          Some(validation_period) => (
              Some(validation_period.from.to_string()),
              Some(validation_period.to.to_string()),
          ),
          None => (None, None),
      };
      Self {
          queryCountThreshold: input.query_count_threshold,
          queryCountThresholdPercentage: input.query_count_threshold_percentage,
          from,
          to,
          // we don't support configuring these, but we can't leave them out
          excludedClients: None,
          excludedOperationNames: None,
          ignoredOperations: None,
          includedVariants: None,
      }
  }
}

impl From<GitContext> for MutationGitContextInput {
  fn from(git_context: GitContext) -> MutationGitContextInput {
      MutationGitContextInput {
          branch: git_context.branch,
          commit: git_context.commit,
          committer: git_context.author,
          remoteUrl: git_context.remote_url,
          message: None,
      }
  }
}
