use std::fmt::Debug;

use apollo_federation_types::{
    build::{BuildErrors, BuildHint, BuildOutput, BuildResult},
    config::FederationVersion,
};
use camino::Utf8PathBuf;
use tap::TapFallible;

use crate::{
    composition::{CompositionError, CompositionSuccess},
    utils::effect::{exec::ExecCommand, read_file::ReadFile},
};

use super::{config::FinalSupergraphConfig, version::SupergraphVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputTarget {
    File(Utf8PathBuf),
    Stdout,
}

impl OutputTarget {
    pub fn align_to_version(self, version: &SupergraphVersion) -> OutputTarget {
        match self {
            OutputTarget::File(path) => {
                if version.supports_output_flag() {
                    OutputTarget::File(path)
                } else {
                    tracing::warn!("This version of supergraph does not support the `--output flag`. Defaulting to `stdout`");
                    OutputTarget::Stdout
                }
            }
            OutputTarget::Stdout => OutputTarget::Stdout,
        }
    }
}

pub struct SupergraphBinary {
    exe: Utf8PathBuf,
    version: SupergraphVersion,
}

impl SupergraphBinary {
    async fn compose(
        &self,
        exec: &impl ExecCommand,
        read_file: &impl ReadFile,
        supergraph_config: FinalSupergraphConfig,
        output_target: OutputTarget,
    ) -> Result<CompositionSuccess, CompositionError> {
        let output_target = output_target.align_to_version(&self.version);
        let mut args = vec!["compose", supergraph_config.path().as_ref()];
        if let OutputTarget::File(output_path) = &output_target {
            args.push(output_path.as_ref());
        }
        let output = exec
            .exec_command(&self.exe, &args)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| CompositionError::Binary {
                error: Box::new(err),
            })?;
        let output = match &output_target {
            OutputTarget::File(path) => {
                read_file
                    .read_file(path)
                    .await
                    .map_err(|err| CompositionError::ReadFile {
                        path: path.clone(),
                        error: Box::new(err),
                    })?
            }
            OutputTarget::Stdout => std::str::from_utf8(&output.stdout)
                .map_err(|err| CompositionError::InvalidOutput {
                    binary: self.exe.clone(),
                    error: Box::new(err),
                })?
                .to_string(),
        };

        self.validate_composition(&output)
    }

    /// Validate that the output of the supergraph binary contains either build errors or build
    /// output, which we'll use later when validating that we have a well-formed composition
    fn validate_supergraph_binary_output(
        &self,
        output: &str,
    ) -> Result<Result<BuildOutput, BuildErrors>, CompositionError> {
        // Attempt to convert the str to a valid composition result; this ensures that we have a
        // well-formed composition. This doesn't necessarily mean we don't have build errors, but
        // we handle those below
        serde_json::from_str::<BuildResult>(output).map_err(|err| CompositionError::InvalidOutput {
            binary: self.exe.clone(),
            error: Box::new(err),
        })
    }

    /// Validates both that the supergraph binary produced a useable output and that that output
    /// represents a valid composition (even if it results in build errors)
    fn validate_composition(
        &self,
        supergraph_binary_output: &str,
    ) -> Result<CompositionSuccess, CompositionError> {
        // Validate the supergraph version is a supported federation version
        let federation_version = self.get_federation_version()?;

        self.validate_supergraph_binary_output(supergraph_binary_output)?
            .map(|build_output| CompositionSuccess {
                hints: build_output.hints,
                supergraph_sdl: build_output.supergraph_sdl,
                federation_version,
            })
            .map_err(|build_errors| CompositionError::Build {
                source: build_errors,
            })
    }

    /// Using the supergraph binary's version to get the supported Federation version
    ///
    /// At the time of writing, these versions are the same. That is, a supergraph binary version
    /// just is the supported Federation version
    fn get_federation_version(&self) -> Result<FederationVersion, CompositionError> {
        self.version
            .clone()
            .try_into()
            .map_err(|error| CompositionError::InvalidInput {
                binary: self.exe.clone(),
                error: Box::new(error),
            })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        process::{ExitStatus, Output},
        str::FromStr,
    };

    use anyhow::Result;
    use apollo_federation_types::{
        build::BuildResult,
        config::{FederationVersion, SupergraphConfig},
    };
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;

    use crate::{
        composition::supergraph::{config::FinalSupergraphConfig, version::SupergraphVersion},
        utils::effect::{exec::MockExecCommand, read_file::MockReadFile},
    };

    use super::{CompositionSuccess, OutputTarget, SupergraphBinary};

    fn fed_one() -> Version {
        Version::from_str("1.0.0").unwrap()
    }

    fn fed_two_eight() -> Version {
        Version::from_str("2.8.0").unwrap()
    }

    fn fed_two_nine() -> Version {
        Version::from_str("2.9.0").unwrap()
    }

    #[fixture]
    fn build_output() -> String {
        "{\"Ok\":{\"supergraphSdl\":\"schema\\n  @link(url: \\\"https://specs.apollo.dev/link/v1.0\\\")\\n  @link(url: \\\"https://specs.apollo.dev/join/v0.3\\\", for: EXECUTION)\\n  @link(url: \\\"https://specs.apollo.dev/tag/v0.3\\\", import: [\\\"@tag\\\"])\\n  @link(url: \\\"https://specs.apollo.dev/inaccessible/v0.2\\\", import: [\\\"@inaccessible\\\"], for: SECURITY)\\n{\\n  query: Query\\n}\\n\\ndirective @inaccessible on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION\\n\\ndirective @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE\\n\\ndirective @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION\\n\\ndirective @join__graph(name: String!, url: String!) on ENUM_VALUE\\n\\ndirective @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE\\n\\ndirective @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR\\n\\ndirective @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION\\n\\ndirective @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA\\n\\ndirective @tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION | SCHEMA\\n\\nscalar join__FieldSet\\n\\nenum join__Graph {\\n  PANDAS @join__graph(name: \\\"pandas\\\", url: \\\"http://localhost:4003\\\")\\n  PRODUCTS @join__graph(name: \\\"products\\\", url: \\\"http://localhost:4002\\\")\\n  USERS @join__graph(name: \\\"users\\\", url: \\\"http://localhost:4001\\\")\\n}\\n\\nscalar link__Import \\n\\nenum link__Purpose {\\n  \\\"\\\"\\\"\\n  `SECURITY` features provide metadata necessary to securely resolve fields.\\n  \\\"\\\"\\\"\\n  SECURITY\\n\\n  \\\"\\\"\\\"\\n  `EXECUTION` features provide metadata necessary for operation execution.\\n  \\\"\\\"\\\"\\n  EXECUTION\\n}\\n\\ntype Panda\\n @join__type(graph: PANDAS)\\n{\\n  name: ID!\\n  favoriteFood: String @tag(name: \\\"nom-nom-nom\\\")\\n}\\n\\ntype Product implements ProductItf & SkuItf\\n  @join__implements(graph: PRODUCTS, interface: \\\"ProductItf\\\")\\n  @join__implements(graph: PRODUCTS, interface: \\\"SkuItf\\\")\\n  @join__type(graph: PRODUCTS, key: \\\"id\\\")\\n  @join__type(graph: PRODUCTS, key: \\\"sku package\\\")\\n @join__type(graph: PRODUCTS, key: \\\"sku variation { id }\\\")\\n{\\n  id: ID! @tag(name: \\\"hi-from-products\\\")\\n  sku: String\\n  package: String\\n  variation: ProductVariation\\n  dimensions: ProductDimension\\n  createdBy: User\\n  hidden: String\\n}\\n\\ntype ProductDimension\\n  @join__type(graph: PRODUCTS)\\n{\\n  size: String\\n  weight: Float\\n}\\n\\ninterface ProductItf implements SkuItf\\n  @join__implements(graph: PRODUCTS, interface: \\\"SkuItf\\\")\\n  @join__type(graph: PRODUCTS)\\n{\\n  id: ID!\\n  sku: String\\n  package: String\\n  variation: ProductVariation\\n  dimensions: ProductDimension\\n  createdBy: User\\n  hidden: String @inaccessible\\n}\\n\\ntype ProductVariation\\n  @join__type(graph: PRODUCTS)\\n{\\n  id: ID!\\n}\\n\\ntype Query\\n  @join__type(graph: PANDAS)\\n  @join__type(graph: PRODUCTS)\\n  @join__type(graph: USERS)\\n{\\n  allPandas: [Panda] @join__field(graph: PANDAS)\\n  panda(name: ID!): Panda @join__field(graph: PANDAS)\\n  allProducts: [ProductItf] @join__field(graph: PRODUCTS)\\n  product(id: ID!): ProductItf @join__field(graph: PRODUCTS)\\n}\\n\\nenum ShippingClass\\n  @join__type(graph: PRODUCTS)\\n{\\n  STANDARD @join__enumValue(graph: PRODUCTS)\\n  EXPRESS @join__enumValue(graph: PRODUCTS)\\n}\\n\\ninterface SkuItf\\n  @join__type(graph: PRODUCTS)\\n{\\n  sku: String\\n}\\n\\ntype User\\n  @join__type(graph: PRODUCTS, key: \\\"email\\\")\\n  @join__type(graph: USERS, key: \\\"email\\\")\\n{\\n  email: ID! @tag(name: \\\"test-from-users\\\")\\n  totalProductsCreated: Int\\n  name: String @join__field(graph: USERS)\\n}\",\"hints\":[{\"message\":\"[UNUSED_ENUM_TYPE]: Enum type \\\"ShippingClass\\\" is defined but unused. It will be included in the supergraph with all the values appearing in any subgraph (\\\"as if\\\" it was only used as an output type).\",\"code\":\"UNUSED_ENUM_TYPE\",\"nodes\":[],\"omittedNodesCount\":0}]}}".to_string()
    }

    #[fixture]
    fn build_result() -> BuildResult {
        serde_json::from_str::<BuildResult>(&build_output()).unwrap()
    }

    #[fixture]
    fn composition_output() -> CompositionSuccess {
        let res = build_result().unwrap();

        CompositionSuccess {
            hints: res.hints,
            supergraph_sdl: res.supergraph_sdl,
            federation_version: FederationVersion::ExactFedTwo(fed_two_eight()),
        }
    }

    #[rstest]
    #[case::fed_one(fed_one(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_eight(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_nine(), OutputTarget::File(Utf8PathBuf::new()))]
    fn test_output_target_file_align_to_version(
        #[case] federation_version: Version,
        #[case] expected_output_target: OutputTarget,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        let given_output_target = OutputTarget::File(Utf8PathBuf::new());
        let result_output_target = given_output_target.align_to_version(&supergraph_version);
        assert_that!(result_output_target).is_equal_to(expected_output_target);
    }

    #[rstest]
    #[case::fed_one(fed_one(), OutputTarget::Stdout)]
    #[case::fed_two_eight(fed_two_eight(), OutputTarget::Stdout)]
    #[case::fed_two_nine(fed_two_nine(), OutputTarget::Stdout)]
    fn test_output_target_stdout_align_to_version(
        #[case] federation_version: Version,
        #[case] expected_output_target: OutputTarget,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        let given_output_target = OutputTarget::Stdout;
        let result_output_target = given_output_target.align_to_version(&supergraph_version);
        assert_that!(result_output_target).is_equal_to(expected_output_target);
    }

    #[rstest]
    #[tokio::test]
    async fn test_compose(
        build_output: String,
        composition_output: CompositionSuccess,
    ) -> Result<()> {
        let supergraph_version = SupergraphVersion::new(fed_two_eight());
        let binary_path = Utf8PathBuf::from_str("/tmp/supergraph")?;

        let supergraph_binary = SupergraphBinary {
            exe: binary_path.clone(),
            version: supergraph_version,
        };

        let supergraph_config_path = Utf8PathBuf::from_str("/tmp/supergraph_config.yaml")?;
        let supergraph_config = FinalSupergraphConfig::new(
            supergraph_config_path,
            SupergraphConfig::new(BTreeMap::new(), None),
        );
        let output_target = OutputTarget::Stdout;

        let mut mock_read_file = MockReadFile::new();
        mock_read_file.expect_read_file().times(0);
        let mut mock_exec = MockExecCommand::new();
        let build_output_blah = build_output.clone();

        mock_exec
            .expect_exec_command()
            .times(1)
            .withf(move |actual_binary_path, actual_arguments| {
                actual_binary_path == &binary_path.clone()
                    && actual_arguments == ["compose", "/tmp/supergraph_config.yaml"]
            })
            .returning(move |_, _| {
                Ok(Output {
                    status: ExitStatus::default(),
                    stdout: build_output_blah.clone().as_bytes().into(),
                    stderr: Vec::default(),
                })
            });
        let result = supergraph_binary
            .compose(
                &mock_exec,
                &mock_read_file,
                supergraph_config,
                output_target,
            )
            .await;

        assert_that!(result).is_ok().is_equal_to(composition_output);

        Ok(())
    }
}
