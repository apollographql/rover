use std::str::FromStr;

use anyhow::Result;
use calm_io::{stderrln, stdoutln};
use camino::Utf8PathBuf;
use clap::{error::ErrorKind as ClapErrorKind, CommandFactory, Parser, ValueEnum};
use rover_client::shared::SdlType;
use rover_std::{Fs, Style, Emoji};
use serde::Serialize;

use crate::{
    cli::{FormatType, Rover},
    command::output::JsonOutput,
    RoverError, RoverOutput, RoverResult,
};

#[derive(Debug, Parser)]
pub struct Output {
    /// The file path to write the command output to.
    #[clap(long)]
    output: OutputType,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum FinalOutputType {
    File(Utf8PathBuf),
    Stdout,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum OutputType {
    LegacyOutputType(FormatType),
    File(Utf8PathBuf),
}

impl FromStr for OutputType {
    type Err = anyhow::Error;

    // TODO: HANDLE THE TRANSFORMATION OF THE JSON LEGACY FORMAT TYPE
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(format_type) = FormatType::from_str(s, true) {
            Ok(Self::LegacyOutputType(format_type))
        } else {
            Ok(Self::File(Utf8PathBuf::from(s)))
        }
    }
}

pub trait RoverOutputTrait {
    fn write_or_print(
        self,
        format_type: FormatType,
        output_type: FinalOutputType,
    ) -> RoverResult<()>;
}

impl RoverOutputTrait for RoverOutput {
    fn write_or_print(
        self,
        format_type: FormatType,
        output_type: FinalOutputType,
    ) -> RoverResult<()> {
        // Format the RoverOutput as either plain text or JSON.
        let output = match format_type {
            FormatType::Plain => self.get_stdout(),
            FormatType::Json => Ok(Some(JsonOutput::from(self.clone()).to_string())),
        };

        // Print the RoverOutput to file or stdout.
        if let Ok(Some(result)) = output {
            match output_type {
                FinalOutputType::File(path) => {
                    let success_heading = Style::Heading.paint(format!("{}The output was printed to", Emoji::Memo));
                    let path_text = Style::Path.paint(&path);
                    Fs::write_file(&path, result)?;
                    stderrln!("{} {}", success_heading, path_text)?;
                }
                FinalOutputType::Stdout => {
                    // Call the appropriate method based on the variant of RoverOutput.
                    let descriptor = match &self {
                        RoverOutput::FetchResponse(fetch_response) => {
                            match fetch_response.sdl.r#type {
                                SdlType::Graph | SdlType::Subgraph { .. } => "SDL",
                                SdlType::Supergraph => "Supergraph SDL",
                            }
                        }
                        RoverOutput::CoreSchema(_) => "CoreSchema",
                        RoverOutput::CompositionResult(_) => "CoreSchema",
                        RoverOutput::TemplateUseSuccess { .. } => "Project generated",
                        RoverOutput::CheckResponse(_) => "Check Result",
                        RoverOutput::AsyncCheckResponse(_) => "Check Started",
                        RoverOutput::Profiles(_) => "Profiles",
                        RoverOutput::Introspection(_) => "Introspection Response",
                        RoverOutput::ReadmeFetchResponse { .. } => "Readme",
                        RoverOutput::GraphPublishResponse { .. } => "Schema Hash",
                        _ => return Ok(()),
                    };
                    if let RoverOutput::GraphPublishResponse { .. } = self {
                        self.print_one_line_descriptor(descriptor)?;
                    } else {
                        self.print_descriptor(descriptor)?;
                    }

                    stdoutln!("{}", &result)?;
                }
            }
        }

        Ok(())
    }
}

impl RoverOutputTrait for RoverError {
    fn write_or_print(self, format_type: FormatType, _: FinalOutputType) -> RoverResult<()> {
        match format_type {
            FormatType::Plain => self.print(),
            FormatType::Json => JsonOutput::from(self).print(),
        }?;

        Ok(())
    }
}

#[derive(Debug, Parser, Serialize)]
pub struct OutputStrategy {
    /// Specify Rover's format type
    #[arg(long = "format", global = true)]
    format_type: Option<FormatType>,

    /// Specify a file to write Rover's output to
    #[arg(long = "output", global = true)]
    output_type: Option<OutputType>,
}

impl OutputStrategy {
    pub fn validate_options(&self) {
        match (&self.format_type, &self.output_type) {
            (Some(_), Some(OutputType::LegacyOutputType(_))) => {
                let mut cmd = Rover::command();
                cmd.error(
                    ClapErrorKind::ArgumentConflict,
                    "The argument '--output' cannot be used with '--format' when '--output' is not a file",
                )
                .exit();
            }
            (None, Some(OutputType::LegacyOutputType(_))) => {
                let warn_prefix = Style::WarningPrefix.paint("WARN:");
                eprintln!("{} The argument '--output' will soon be deprecated. Please use the '--format' argument to specify the output type.", warn_prefix);
            }
            _ => (),
        }
    }

    pub fn get_strategy(&self) -> (FormatType, FinalOutputType) {
        let output_type = self.output_type.clone();

        match (&self.format_type, output_type) {
            (None, None) | (None, Some(OutputType::LegacyOutputType(FormatType::Plain))) => {
                (FormatType::Plain, FinalOutputType::Stdout)
            }
            (None, Some(OutputType::LegacyOutputType(FormatType::Json))) => {
                (FormatType::Json, FinalOutputType::Stdout)
            }
            (None, Some(OutputType::File(path))) => {
                (FormatType::Plain, FinalOutputType::File(path))
            }
            (Some(FormatType::Plain), None)
            | (Some(FormatType::Plain), Some(OutputType::LegacyOutputType(FormatType::Plain))) => {
                (FormatType::Plain, FinalOutputType::Stdout)
            }
            (Some(FormatType::Plain), Some(OutputType::LegacyOutputType(FormatType::Json))) => {
                (FormatType::Json, FinalOutputType::Stdout)
            }
            (Some(FormatType::Plain), Some(OutputType::File(path))) => {
                (FormatType::Plain, FinalOutputType::File(path))
            }
            (Some(FormatType::Json), None)
            | (Some(FormatType::Json), Some(OutputType::LegacyOutputType(_))) => {
                (FormatType::Json, FinalOutputType::Stdout)
            }
            (Some(FormatType::Json), Some(OutputType::File(path))) => {
                (FormatType::Json, FinalOutputType::File(path))
            }
        }
    }

    pub fn write_rover_output<T>(&self, output_trait: T) -> Result<(), RoverError>
    where
        T: RoverOutputTrait,
    {
        let (format_type, output_type) = self.get_strategy();
        output_trait.write_or_print(format_type, output_type)?;
        Ok(())
    }

    pub fn get_json(&self) -> bool {
        matches!(self.format_type, Some(FormatType::Json))
            || matches!(
                self.output_type,
                Some(OutputType::LegacyOutputType(FormatType::Json))
            )
    }
}
