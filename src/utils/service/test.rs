use std::{convert::Infallible, pin::Pin};

use futures::Future;
use tower::Service;

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct FakeError(Box<dyn std::error::Error + Send + Sync>);

impl From<Box<dyn std::error::Error + Send + Sync>> for FakeError {
    fn from(value: Box<dyn std::error::Error + Send + Sync>) -> Self {
        FakeError(value)
    }
}

#[derive(Clone)]
pub struct FakeMakeService<S: Clone> {
    service: S,
}

impl<S: Clone> FakeMakeService<S> {
    pub fn new(service: S) -> FakeMakeService<S> {
        FakeMakeService { service }
    }
}

impl<S, T> Service<T> for FakeMakeService<S>
where
    S: Clone + Send + 'static,
{
    type Response = S;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: T) -> Self::Future {
        let service = self.service.clone();
        let fut = async move { Ok(service) };
        Box::pin(fut)
    }
}
