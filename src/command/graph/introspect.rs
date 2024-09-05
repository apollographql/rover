use clap::Parser;
use rover_http::HttpService;
use serde::Serialize;
use std::time::Duration;

use rover_client::{operations::graph::introspect, IntrospectionConfig};

use crate::{
    options::{IntrospectOpts, OutputOpts},
    RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    pub opts: IntrospectOpts,
}

impl Introspect {
    pub async fn run(
        &self,
        http_service: &HttpService,
        output_opts: &OutputOpts,
        retry_period: Option<Duration>,
    ) -> RoverResult<RoverOutput> {
        if self.opts.watch {
            self.exec_and_watch(http_service, output_opts, retry_period)
                .await
        } else {
            let sdl = self.exec(http_service, true, retry_period).await?;
            Ok(RoverOutput::Introspection(sdl))
        }
    }

    pub async fn exec(
        &self,
        http_service: &HttpService,
        should_retry: bool,
        retry_period: Option<Duration>,
    ) -> RoverResult<String> {
        let http_service = http_service.get().await;
        let config = IntrospectionConfig::builder()
            .endpoint(self.opts.endpoint.clone())
            .and_headers(self.opts.headers.clone())
            .should_retry(should_retry)
            .and_retry_period(retry_period)
            .build()?;

        Ok(introspect::run(config, http_service).await?.schema_sdl)
    }

    pub async fn exec_and_watch(
        &self,
        http_service_factory: &HttpService,
        output_opts: &OutputOpts,

        retry_period: Option<Duration>,
    ) -> ! {
        self.opts
            .exec_and_watch(
                || self.exec(http_service_factory, false, retry_period),
                output_opts,
            )
            .await
    }
}
