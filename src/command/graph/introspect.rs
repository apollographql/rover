use std::{collections::HashMap, time::Duration};

use clap::Parser;
use reqwest::Client;
use rover_client::operations::graph::introspect::{
    self, GraphIntrospectInput, sdl_to_introspection_json,
};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    cli::RoverOutputFormatKind,
    options::{IntrospectOpts, OutputOpts},
};

#[derive(Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    pub opts: IntrospectOpts,

    /// Skip auto-detection and use the pre-October-2021 introspection query
    /// directly, omitting `includeDeprecated` on `args`/`inputFields` and
    /// `isDeprecated`/`deprecationReason` on `__InputValue`. By default
    /// Rover runs the modern query first and automatically retries with the
    /// legacy query when the server rejects it (HTTP 422, or GraphQL body
    /// errors mentioning the unknown fields). Use this flag when you already
    /// know the target server is pre-spec and want to avoid the extra
    /// round-trip.
    #[arg(long)]
    #[serde(skip_serializing)]
    pub legacy_introspection_query: bool,
}

impl Introspect {
    pub async fn run(
        &self,
        client: Client,
        output_opts: &OutputOpts,
        retry_period: Duration,
    ) -> RoverResult<RoverOutput> {
        if self.opts.watch {
            self.exec_and_watch(&client, output_opts, retry_period)
                .await
        } else {
            let sdl = self.exec(&client, true, retry_period).await?;
            Self::sdl_to_output(sdl, output_opts.format_kind)
        }
    }

    pub async fn exec(
        &self,
        client: &Client,
        should_retry: bool,
        retry_period: Duration,
    ) -> RoverResult<String> {
        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if let Some(arg_headers) = &self.opts.headers {
            for (header_key, header_value) in arg_headers {
                headers.insert(header_key.to_string(), header_value.to_string());
            }
        };

        Ok(introspect::run(
            GraphIntrospectInput {
                headers,
                endpoint: self.opts.endpoint.clone(),
                should_retry,
                retry_period,
                use_legacy_introspection_query: self.legacy_introspection_query,
            },
            client,
        )
        .await?
        .schema_sdl)
    }

    fn sdl_to_output(sdl: String, format_kind: RoverOutputFormatKind) -> RoverResult<RoverOutput> {
        match format_kind {
            RoverOutputFormatKind::Json => Ok(RoverOutput::IntrospectionJson(
                sdl_to_introspection_json(&sdl)?,
            )),
            RoverOutputFormatKind::Plain => Ok(RoverOutput::Introspection(sdl)),
        }
    }

    pub async fn exec_and_watch(
        &self,
        client: &Client,
        output_opts: &OutputOpts,

        retry_period: Duration,
    ) -> ! {
        self.opts
            .exec_and_watch(
                || self.exec(client, false, retry_period),
                |sdl| Self::sdl_to_output(sdl, output_opts.format_kind),
                output_opts,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::Introspect;
    use crate::{RoverOutput, cli::RoverOutputFormatKind, options::JsonOutput};

    const SWAPI_SDL: &str = include_str!(
        "../../../crates/rover-client/src/operations/graph/introspect/fixtures/swapi.graphql"
    );
    const SWAPI_REFERENCE_JSON: &str = include_str!(
        "../../../crates/rover-client/src/operations/graph/introspect/fixtures/swapi-introspection.json"
    );

    #[test]
    fn format_json_matches_swapi_reference_structurally() {
        use rover_client::operations::graph::introspect::assert_structural_parity;

        let reference: serde_json::Value = serde_json::from_str(SWAPI_REFERENCE_JSON).unwrap();
        let output =
            Introspect::sdl_to_output(SWAPI_SDL.to_string(), RoverOutputFormatKind::Json).unwrap();

        let RoverOutput::IntrospectionJson(actual) = output else {
            panic!("expected IntrospectionJson, got {output:?}");
        };

        assert_structural_parity(&actual, &reference);
    }

    #[test]
    fn format_json_round_trips_through_sdl_to_valid_schema() {
        use rover_client::operations::graph::introspect::introspection_json_to_validated_sdl;

        let output =
            Introspect::sdl_to_output(SWAPI_SDL.to_string(), RoverOutputFormatKind::Json).unwrap();

        let RoverOutput::IntrospectionJson(introspection) = output else {
            panic!("expected IntrospectionJson, got {output:?}");
        };

        introspection_json_to_validated_sdl(&introspection).unwrap();
    }

    #[test]
    fn format_json_returns_introspection_object() {
        let output =
            Introspect::sdl_to_output(SWAPI_SDL.to_string(), RoverOutputFormatKind::Json).unwrap();

        let RoverOutput::IntrospectionJson(v) = output else {
            panic!("expected IntrospectionJson, got {output:?}");
        };
        assert!(v["__schema"].is_object());
        assert!(v.get("data").is_none());
        assert!(v.get("errors").is_none());
    }

    #[test]
    fn format_json_envelope_exposes_schema_under_data() {
        let output =
            Introspect::sdl_to_output(SWAPI_SDL.to_string(), RoverOutputFormatKind::Json).unwrap();

        let RoverOutput::IntrospectionJson(v) = output else {
            panic!("expected IntrospectionJson");
        };

        let envelope = JsonOutput::from(&RoverOutput::IntrospectionJson(v.clone()));
        let envelope_json: serde_json::Value = serde_json::from_str(&envelope.to_string()).unwrap();
        assert_eq!(
            envelope_json["data"]["introspection_response"]["__schema"],
            v["__schema"]
        );
    }

    #[test]
    fn format_plain_returns_sdl() {
        let output =
            Introspect::sdl_to_output(SWAPI_SDL.to_string(), RoverOutputFormatKind::Plain).unwrap();

        let RoverOutput::Introspection(ref sdl) = output else {
            panic!("expected Introspection, got {output:?}");
        };
        assert_eq!(sdl, SWAPI_SDL);
        assert_eq!(output.get_stdout().unwrap().unwrap(), SWAPI_SDL);
    }
}
