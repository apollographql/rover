use clap::Parser;
use crate::{
    RoverResult,
    utils::{effect::read_stdin::ReadStdin, parsers::FileDescriptorType},
};

const EXAMPLE_SCHEMA: &str = "type Query { helloWorld: String }";
const EXAMPLE_URL: &str = "https://example.com";

#[derive(Debug, Parser)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[arg(long, short = 's')]
    schema: FileDescriptorType,
}

pub struct FileWithMetadata {
    pub schema: String,
    pub file_path: String,
}

impl SchemaOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<String> {
        self.schema
            .read_file_descriptor(file_description, read_stdin_impl)
    }

    pub(crate) fn read_file_descriptor_with_metadata(
        &self,
        file_description: &str,
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<FileWithMetadata> {
        match self
            .schema
            .read_file_descriptor(file_description, read_stdin_impl)
        {
            Ok(proposed_schema) => Ok(FileWithMetadata {
                schema: proposed_schema,
                file_path: match &self.schema {
                    FileDescriptorType::Stdin => "stdin".to_owned(),
                    FileDescriptorType::File(file_path) => file_path.to_string(),
                },
            }),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Parser)]
pub struct OptionalSchemaOpt {
    /// The schema file to publish. You can pass `-` to use stdin instead of a file.
    #[arg(
        long,
        short = 's',
        required_unless_present = "use_example_schema",
        conflicts_with = "use_example_schema"
    )]
    schema: Option<FileDescriptorType>,

    #[arg(long)]
    use_example_schema: bool,
}

impl OptionalSchemaOpt {
    pub fn is_using_example_schema(&self) -> bool {
        self.use_example_schema
    }

    pub fn example_schema() -> &'static str {
        EXAMPLE_SCHEMA
    }

    pub fn example_url() -> &'static str {
        EXAMPLE_URL
    }

    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<String> {
        self.schema
            .as_ref()
            .expect("schema is required unless use_example_schema is set")
            .read_file_descriptor(file_description, read_stdin_impl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[derive(Debug, Parser)]
    struct TestCmd {
        #[clap(flatten)]
        schema: OptionalSchemaOpt,

        #[arg(long)]
        name: String,
    }

    #[test]
    fn test_use_example_schema_returns_correct_values() {
        let cmd = TestCmd::try_parse_from([
            "test",
            "--name",
            "my-subgraph",
            "--use-example-schema",
        ])
        .unwrap();

        assert!(cmd.schema.is_using_example_schema());
        assert_eq!(
            OptionalSchemaOpt::example_schema(),
            "type Query { helloWorld: String }"
        );
        assert_eq!(OptionalSchemaOpt::example_url(), "https://example.com");
        assert_eq!(cmd.name, "my-subgraph");
    }

    #[test]
    fn test_schema_flag_works_without_example() {
        let cmd = TestCmd::try_parse_from([
            "test",
            "--name",
            "my-subgraph",
            "--schema",
            "./my-schema.graphql",
        ])
        .unwrap();

        assert!(!cmd.schema.is_using_example_schema());
        assert!(cmd.schema.schema.is_some());
        assert_eq!(cmd.name, "my-subgraph");
    }

    #[test]
    fn test_error_when_both_schema_and_use_example_schema_provided() {
        let result = TestCmd::try_parse_from([
            "test",
            "--name",
            "my-subgraph",
            "--schema",
            "./my-schema.graphql",
            "--use-example-schema",
        ]);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("cannot be used with"),
            "Expected conflict error, got: {err}"
        );
    }

    #[test]
    fn test_error_when_neither_schema_nor_use_example_schema_provided() {
        let result = TestCmd::try_parse_from(["test", "--name", "my-subgraph"]);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("--schema") || err.contains("required"),
            "Expected missing argument error, got: {err}"
        );
    }

    #[test]
    fn test_both_options_appear_in_help() {
        let help = TestCmd::command().render_help().to_string();

        assert!(
            help.contains("--schema"),
            "Help should contain --schema option"
        );
        assert!(
            help.contains("--use-example-schema"),
            "Help should contain --use-example-schema option"
        );
        assert!(
            help.contains("-s"),
            "Help should contain -s short option for schema"
        );
    }
}
