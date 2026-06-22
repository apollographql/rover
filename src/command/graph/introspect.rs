use std::{collections::HashMap, time::Duration};

use clap::Parser;
use reqwest::Client;
use rover_client::operations::graph::introspect::{
    self, GraphIntrospectInput, sdl_to_introspection_json,
};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
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

    /// Return the schema as GraphQL introspection JSON (`{ "__schema": ... }`)
    /// instead of SDL, matching the legacy `apollo schema:download` format.
    #[arg(long)]
    #[serde(skip_serializing)]
    pub introspection_json: bool,
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
            Ok(self.sdl_to_output(sdl)?)
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

    fn sdl_to_output(&self, sdl: String) -> RoverResult<RoverOutput> {
        if self.introspection_json {
            Ok(RoverOutput::IntrospectionJson(sdl_to_introspection_json(
                &sdl,
            )?))
        } else {
            Ok(RoverOutput::Introspection(sdl))
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
                |sdl| self.sdl_to_output(sdl),
                output_opts,
            )
            .await
    }
}
