use std::{fmt, io, str::FromStr};

use anyhow::Result;
use calm_io::{stderrln, stdoutln};
use camino::Utf8PathBuf;
use clap::{error::ErrorKind as ClapErrorKind, CommandFactory, Parser, ValueEnum};
use rover_std::{Emoji, Fs, Style};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    cli::{Rover, RoverOutputFormatKind},
    RoverError, RoverOutput, RoverResult,
};

#[derive(Debug, Parser)]
pub struct Output {
    /// The file path to write the command output to.
    #[clap(long)]
    output: OutputOpt,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum RoverOutputDestination {
    File(Utf8PathBuf),
    Stdout,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum OutputOpt {
    LegacyOutputType(RoverOutputFormatKind),
    File(Utf8PathBuf),
}

impl FromStr for OutputOpt {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(format_kind) = RoverOutputFormatKind::from_str(s, true) {
            Ok(Self::LegacyOutputType(format_kind))
        } else {
            Ok(Self::File(Utf8PathBuf::from(s)))
        }
    }
}

pub trait RoverPrinter {
    fn write_or_print(self, output_opts: &OutputOpts) -> RoverResult<()>;
}

impl RoverPrinter for RoverOutput {
    fn write_or_print(self, output_opts: &OutputOpts) -> RoverResult<()> {
        let (format_kind, output_destination) = output_opts.get_format_and_strategy();

        // Format the RoverOutput as either plain text or JSON.
        let output = match format_kind {
            RoverOutputFormatKind::Plain => self.get_stdout(),
            RoverOutputFormatKind::Json => Ok(Some(JsonOutput::from(self.clone()).to_string())),
        };

        // Print the RoverOutput to file or stdout.
        if let Ok(Some(result)) = output {
            match output_destination {
                RoverOutputDestination::File(path) => {
                    let success_heading = Style::Heading.paint(format!(
                        "{}{} was printed to",
                        Emoji::Memo,
                        self.descriptor().unwrap_or("The output")
                    ));
                    let path_text = Style::Path.paint(&path);
                    Fs::write_file(&path, result)?;
                    stderrln!("{} {}", success_heading, path_text)?;
                }
                RoverOutputDestination::Stdout => {
                    // Call the appropriate method based on the variant of RoverOutput.
                    if let RoverOutput::GraphPublishResponse { .. } = self {
                        self.print_one_line_descriptor()?;
                    } else {
                        self.print_descriptor()?;
                    }

                    stdoutln!("{}", &result)?;
                }
            }
        }

        Ok(())
    }
}

impl RoverPrinter for RoverError {
    fn write_or_print(self, output_opts: &OutputOpts) -> RoverResult<()> {
        let (format_kind, output_destination) = output_opts.get_format_and_strategy();
        match format_kind {
            RoverOutputFormatKind::Plain => self.print(),
            RoverOutputFormatKind::Json => {
                let json = JsonOutput::from(self);
                match output_destination {
                    RoverOutputDestination::File(file) => {
                        let success_heading = Style::Heading
                            .paint(format!("{}Error JSON was printed to", Emoji::Memo,));
                        Fs::write_file(&file, json.to_string())?;
                        stderrln!("{} {}", success_heading, file)?;
                    }
                    RoverOutputDestination::Stdout => json.print()?,
                }
                Ok(())
            }
        }?;

        Ok(())
    }
}

#[derive(Debug, Parser, Serialize)]
pub struct OutputOpts {
    /// Specify Rover's format type
    #[arg(long = "format", global = true)]
    format_kind: Option<RoverOutputFormatKind>,

    /// Specify a file to write Rover's output to
    #[arg(long = "output", short = 'o', global = true)]
    output_file: Option<OutputOpt>,
}

impl OutputOpts {
    /// Validates the argument group, exiting early if there are conflicts.
    /// This should be called at the start of the application.
    pub fn validate_options(&self) {
        match (&self.format_kind, &self.output_file) {
            (Some(_), Some(OutputOpt::LegacyOutputType(_))) => {
                let mut cmd = Rover::command();
                cmd.error(
                        ClapErrorKind::ArgumentConflict,
                        "The argument '--output' cannot be used with '--format' when '--output' is not a file",
                    )
                    .exit();
            }
            (None, Some(OutputOpt::LegacyOutputType(_))) => {
                let warn_prefix = Style::WarningPrefix.paint("WARN:");
                let output_argument = Style::Command.paint("'--output [json|plain]'");
                let format_argument = Style::Command.paint("'--format [json|plain]'");
                eprintln!("{} Support for {output_argument} will be removed in a future version of Rover. Use {format_argument} instead.", warn_prefix);
            }
            // there are default options, so if nothing is passed, print no errors or warnings
            _ => (),
        }
    }

    /// Handle output and errors from a Rover command.
    pub fn handle_output<T>(&self, rover_command_output: T) -> RoverResult<()>
    where
        T: RoverPrinter,
    {
        rover_command_output.write_or_print(self)
    }

    /// Get the format (plain/json) and strategy (stdout/file)
    pub fn get_format_and_strategy(&self) -> (RoverOutputFormatKind, RoverOutputDestination) {
        let output_type = self.output_file.clone();

        match (&self.format_kind, output_type) {
            (None, None)
            | (None, Some(OutputOpt::LegacyOutputType(RoverOutputFormatKind::Plain))) => {
                (RoverOutputFormatKind::Plain, RoverOutputDestination::Stdout)
            }
            (None, Some(OutputOpt::LegacyOutputType(RoverOutputFormatKind::Json))) => {
                (RoverOutputFormatKind::Json, RoverOutputDestination::Stdout)
            }
            (None, Some(OutputOpt::File(path))) => (
                RoverOutputFormatKind::Plain,
                RoverOutputDestination::File(path),
            ),
            (Some(RoverOutputFormatKind::Plain), None)
            | (
                Some(RoverOutputFormatKind::Plain),
                Some(OutputOpt::LegacyOutputType(RoverOutputFormatKind::Plain)),
            ) => (RoverOutputFormatKind::Plain, RoverOutputDestination::Stdout),
            (
                Some(RoverOutputFormatKind::Plain),
                Some(OutputOpt::LegacyOutputType(RoverOutputFormatKind::Json)),
            ) => (RoverOutputFormatKind::Json, RoverOutputDestination::Stdout),
            (Some(RoverOutputFormatKind::Plain), Some(OutputOpt::File(path))) => (
                RoverOutputFormatKind::Plain,
                RoverOutputDestination::File(path),
            ),
            (Some(RoverOutputFormatKind::Json), None)
            | (Some(RoverOutputFormatKind::Json), Some(OutputOpt::LegacyOutputType(_))) => {
                (RoverOutputFormatKind::Json, RoverOutputDestination::Stdout)
            }
            (Some(RoverOutputFormatKind::Json), Some(OutputOpt::File(path))) => (
                RoverOutputFormatKind::Json,
                RoverOutputDestination::File(path),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonOutput {
    json_version: JsonVersion,
    data: JsonData,
    error: Value,
}

impl JsonOutput {
    fn success(data: Value, error: Value, json_version: JsonVersion) -> JsonOutput {
        JsonOutput {
            json_version,
            data: JsonData::success(data),
            error,
        }
    }

    fn failure(data: Value, error: Value, json_version: JsonVersion) -> JsonOutput {
        JsonOutput {
            json_version,
            data: JsonData::failure(data),
            error,
        }
    }

    fn print(&self) -> io::Result<()> {
        stdoutln!("{}", self)
    }
}

impl fmt::Display for JsonOutput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", json!(self))
    }
}

impl From<RoverError> for JsonOutput {
    fn from(error: RoverError) -> Self {
        let data_json = error.get_internal_data_json();
        let error_json = error.get_internal_error_json();
        JsonOutput::failure(data_json, error_json, error.get_json_version())
    }
}

impl From<RoverOutput> for JsonOutput {
    fn from(output: RoverOutput) -> Self {
        let data = output.get_internal_data_json();
        let error = output.get_internal_error_json();
        JsonOutput::success(data, error, output.get_json_version())
    }
}

#[derive(Debug, Clone, Serialize)]
struct JsonData {
    #[serde(flatten)]
    inner: Value,
    success: bool,
}

impl JsonData {
    fn success(inner: Value) -> JsonData {
        JsonData {
            inner,
            success: true,
        }
    }

    fn failure(inner: Value) -> JsonData {
        JsonData {
            inner,
            success: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum JsonVersion {
    #[serde(rename = "1")]
    One,
}

impl Default for JsonVersion {
    fn default() -> Self {
        JsonVersion::One
    }
}
