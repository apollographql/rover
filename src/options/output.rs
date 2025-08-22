use std::{
    fmt,
    io::{self, IsTerminal},
    path,
};

use calm_io::{stderrln, stdoutln};
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use serde_json::{Value, json};

use rover_std::{Fs, Style};
use tokio::sync::mpsc::UnboundedSender;

use crate::{RoverError, RoverOutput, RoverResult, cli::RoverOutputFormatKind};

pub trait RoverPrinter {
    fn write_or_print(self, output_opts: &OutputOpts) -> RoverResult<()>;
}

impl RoverPrinter for RoverOutput {
    fn write_or_print(self, output_opts: &OutputOpts) -> RoverResult<()> {
        // Format the RoverOutput as either plain text or JSON.
        let output = match output_opts.format_kind {
            RoverOutputFormatKind::Plain => self.get_stdout(),
            RoverOutputFormatKind::Json => Ok(Some(JsonOutput::from(self.clone()).to_string())),
        };

        // Print the RoverOutput to file or stdout.
        if let Ok(Some(result)) = output {
            match &output_opts.output_file {
                Some(path) => {
                    let success_heading = Style::Heading.paint(format!(
                        "{} was printed to",
                        self.descriptor().unwrap_or("The output")
                    ));
                    let path_text = Style::Path.paint(path);
                    Fs::write_file(path, result)?;
                    stderrln!("{} {}", success_heading, path_text)?;
                }
                None => {
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
        match output_opts.format_kind {
            RoverOutputFormatKind::Plain => self.print(),
            RoverOutputFormatKind::Json => {
                let json = JsonOutput::from(self);
                match &output_opts.output_file {
                    Some(file) => {
                        let success_heading = Style::Heading.paint("Error JSON was printed to");
                        Fs::write_file(file, json.to_string())?;
                        stderrln!("{} {}", success_heading, file)?;
                    }
                    None => json.print()?,
                }
                Ok(())
            }
        }?;

        Ok(())
    }
}

/// The output expected by the channel used for OutputOpts
pub enum OutputChannelKind {
    /// SDL as a String, often via introspection
    Sdl(String),
}

#[derive(Debug, Parser, Serialize, Default)]
pub struct OutputOpts {
    /// Specify Rover's format type
    #[arg(long = "format", global = true, default_value_t)]
    pub format_kind: RoverOutputFormatKind,

    /// Specify a file to write Rover's output to
    #[arg(long = "output", short = 'o', global = true, value_parser = Self::parse_absolute_path)]
    pub output_file: Option<Utf8PathBuf>,

    #[arg(skip)]
    #[serde(skip_serializing)]
    pub channel: Option<UnboundedSender<OutputChannelKind>>,
}

impl OutputOpts {
    /// Sets the `NO_COLOR` env var if the output is not a terminal or if the output is a file.
    pub fn set_no_color(&self) {
        if !io::stdout().is_terminal() || self.output_file.is_some() {
            unsafe {
                std::env::set_var("NO_COLOR", "true");
            }
        }
    }

    /// Handle output and errors from a Rover command.
    pub fn handle_output<T>(&self, rover_command_output: T) -> RoverResult<()>
    where
        T: RoverPrinter,
    {
        rover_command_output.write_or_print(self)
    }

    /// Handle the parsing of output file to ensure we get an absolute path every time
    pub fn parse_absolute_path(path_input: &str) -> Result<Utf8PathBuf, clap::Error> {
        let starter = Utf8PathBuf::from(path_input);
        let absolute_path = path::absolute(starter.as_std_path())?.to_path_buf();
        Ok(Utf8PathBuf::from_path_buf(absolute_path).unwrap())
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

#[derive(Debug, Default, Clone, Serialize)]
pub enum JsonVersion {
    #[serde(rename = "1")]
    #[default]
    One,
    #[serde(rename = "2")]
    Two,
}
