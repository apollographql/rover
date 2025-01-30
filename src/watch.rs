use std::time::Duration;

use buildstructor::Builder;
use tap::TapFallible;
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceExt};

use crate::subtask::SubtaskHandleUnit;

#[derive(Builder)]
pub struct Watch<S>
where
    S: Service<()>,
{
    polling_interval: Duration,
    service: S,
}

impl<S> SubtaskHandleUnit for Watch<S>
where
    S: Service<()> + Send + Clone + 'static,
    S::Response: PartialEq + Send + Clone,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send,
{
    type Output = Result<S::Response, S::Error>;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let mut service = self.service.clone();
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::task::spawn(async move {
            let cancellation_token = cancellation_token.clone();
            let mut interval = tokio::time::interval(self.polling_interval);
            cancellation_token
                .run_until_cancelled(async move {
                    let service = service.ready().await.unwrap();
                    let mut last_result: Option<Result<S::Response, String>> = None;
                    loop {
                        interval.tick().await;
                        match service.call(()).await {
                            Ok(output) => {
                                let mut was_updated = true;
                                if let Some(Ok(last)) = last_result {
                                    if last == output {
                                        was_updated = false
                                    }
                                }
                                if was_updated {
                                    let _ = sender
                                        .send(Ok(output.clone()))
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                }
                                last_result = Some(Ok(output));
                            }
                            Err(error) => {
                                let mut was_updated = true;
                                let e = error.to_string();
                                if let Some(Err(last)) = last_result {
                                    if last == e {
                                        was_updated = false;
                                    }
                                }
                                if was_updated {
                                    let _ = sender
                                        .send(Err(error))
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                }
                                last_result = Some(Err(e));
                            }
                        }
                    }
                })
                .await;
        });
    }
}
