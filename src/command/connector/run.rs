use std::path::PathBuf;

use camino::Utf8PathBuf;
use clap::Parser;
use rover_std::Style;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::utils::table;
use crate::{RoverOutput, RoverResult};

#[derive(thiserror::Error, Debug)]
pub enum RunConnectorError {
    #[error("Failed to run the connectors binary")]
    Binary { error: String },

    #[error("The connectors binary exited with errors.\nStdout: {}\nStderr: {}", .stdout, .stderr)]
    BinaryExit {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[error("Failed to parse output of `{binary} connectors run`\n{error}")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
}

#[derive(Debug, Parser, Clone, Serialize)]
pub struct RunConnector {
    #[arg(short = 'p', long = "path")]
    schema_path: PathBuf,
    #[arg(short = 'c', long = "connector-id")]
    connector_id: String,
    #[arg(short = 'v', long = "variables")]
    variables: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
pub struct RunConnectorOutput {
    request: Option<Request>,
    response: Option<Response>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Request {
    // TODO: Make a dedicated struct rather than Rover relying on apollo-federation
    method: String,
    uri: String,
    headers: Value,
    problems: Vec<Problem>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Response {
    body: Value,
    status: u16,
    headers: Value,
    problems: Vec<Problem>,
    mapped_data: Value,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Problem {
    message: String,
    path: String,
    count: i32,
    location: String,
}

impl RunConnector {
    pub async fn run(&self, supergraph_binary: SupergraphBinary) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let result = supergraph_binary
            .run_connector(
                &exec_command_impl,
                self.schema_path.clone(),
                self.connector_id.clone(),
                self.variables.clone(),
            )
            .await?;
        Ok(result)
    }

    pub fn format_output(output: &RunConnectorOutput) -> String {
        if let Some(error) = &output.error {
            let error_prefix = Style::ErrorPrefix.paint("ERROR:");
            return format!("{} {error}", error_prefix);
        }

        let mut request_content = "".to_string();
        if let Some(request) = &output.request {
            let request_header = Style::SuccessHeading.paint("Request");

            let mut request_table = table::get_table();
            request_table.add_row(vec![Style::Heading.paint("Method"), request.method.clone()]);
            request_table.add_row(vec![Style::Heading.paint("Uri"), request.uri.clone()]);

            request_content = format!("{request_header}\n{request_table}");
        }

        let mut request_headers_content = "".to_string();
        if let Some(request) = &output.request
            && let Some(header_object) = request.headers.as_object()
            && !header_object.is_empty()
        {
            let request_headers_header = Style::SuccessHeading.paint("Request Headers");

            let mut request_headers_table = table::get_table();
            request_headers_table.set_header(vec![
                Style::Heading.paint("Header"),
                Style::Heading.paint("Value"),
            ]);

            for (header_name, header_value) in header_object {
                request_headers_table.add_row(vec![
                    header_name,
                    &header_value.as_str().unwrap_or_default().to_string(),
                ]);
            }

            request_headers_content =
                format!("\n\n{request_headers_header}\n{request_headers_table}");
        }

        let mut request_problems_content = "".to_string();
        if let Some(request) = &output.request
            && !request.problems.is_empty()
        {
            let request_problems_header = Style::WarningHeading.paint("Request Mapping Problems");

            let mut request_problems_table = table::get_table();
            request_problems_table.set_header(vec![
                Style::Heading.paint("Message"),
                Style::Heading.paint("Path"),
                Style::Heading.paint("Count"),
                Style::Heading.paint("Location"),
            ]);

            for problem in &request.problems {
                request_problems_table.add_row(vec![
                    problem.message.clone(),
                    problem.path.clone(),
                    problem.count.to_string(),
                    problem.location.clone(),
                ]);
            }

            request_problems_content =
                format!("\n\n{request_problems_header}\n{request_problems_table}");
        }

        let mut response_content = "".to_string();
        if let Some(response) = &output.response {
            let response_header = Style::SuccessHeading.paint("Response");

            let mut response_table = table::get_table();
            response_table.set_header(vec![
                Style::Heading.paint("Status"),
                Style::Heading.paint("Body"),
                Style::Heading.paint("Mapped Data"),
            ]);

            response_table.add_row(vec![
                response.status.to_string(),
                serde_json::to_string_pretty(&response.body).unwrap_or_default(),
                serde_json::to_string_pretty(&response.mapped_data).unwrap_or_default(),
            ]);

            response_content = format!("\n\n{response_header}\n{response_table}");
        }

        let mut response_headers_content = "".to_string();
        if let Some(response) = &output.response
            && let Some(header_object) = response.headers.as_object()
            && !header_object.is_empty()
        {
            let response_headers_header = Style::SuccessHeading.paint("Response Headers");

            let mut response_headers_table = table::get_table();
            response_headers_table.set_header(vec![
                Style::Heading.paint("Header"),
                Style::Heading.paint("Value"),
            ]);

            for (header_name, header_value) in header_object {
                response_headers_table.add_row(vec![
                    header_name,
                    &header_value.as_str().unwrap_or_default().to_string(),
                ]);
            }

            response_headers_content =
                format!("\n\n{response_headers_header}\n{response_headers_table}");
        }

        let mut response_problems_content = "".to_string();
        if let Some(response) = &output.response
            && !response.problems.is_empty()
        {
            let response_problems_header = Style::WarningHeading.paint("Response Mapping Problems");

            let mut response_problems_table = table::get_table();
            response_problems_table.set_header(vec![
                Style::Heading.paint("Message"),
                Style::Heading.paint("Path"),
                Style::Heading.paint("Count"),
                Style::Heading.paint("Location"),
            ]);

            for problem in &response.problems {
                response_problems_table.add_row(vec![
                    problem.message.clone(),
                    problem.path.clone(),
                    problem.count.to_string(),
                    problem.location.clone(),
                ]);
            }

            response_problems_content =
                format!("\n\n{response_problems_header}\n{response_problems_table}");
        }

        format!(
            "{request_content}{request_headers_content}{request_problems_content}{response_content}{response_headers_content}{response_problems_content}"
        )
    }
}
