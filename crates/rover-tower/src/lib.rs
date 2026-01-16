use std::pin::Pin;

use futures::Future;

pub mod service;
#[cfg(any(test, feature = "test"))]
pub mod test;

pub type ResponseFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[macro_export]
macro_rules! default_poll_ready {
    () => {
        fn poll_ready(
            &mut self,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    };
}
