pub use mockall as mockall_;
pub use pastey as paste_;

#[macro_export]
macro_rules! mock_service {
    ($name:ident, $request:ty, $response:ty, $error:ty) => {
        $crate::test::paste_::paste! {
            $crate::test::mockall_::mock! {
                #[allow(dead_code)]
                #[derive(Debug)]
                pub [<$name Service>] {}
                #[allow(dead_code)]
                impl tower::Service<$request> for [<$name Service>] {
                    type Response = $response;
                    type Error = $error;
                    type Future = futures::future::Ready<Result<$response, $error>>;
                    #[allow(clippy::needless_lifetimes)]
                    fn poll_ready<'a>(
                        &mut self,
                        _cx: &mut std::task::Context<'a>,
                    ) -> std::task::Poll<Result<(), $error>>;

                    fn call(&mut self, req: $request) -> futures::future::Ready<Result<$response, $error>>;
                }
            }
        }
    };
}

use std::{pin::Pin, sync::Arc};

use futures::{lock::Mutex, Future};
pub use mock_service;

#[macro_export]
macro_rules! expect_poll_ready {
    ($name:expr,$times:literal) => {
        $name
            .expect_poll_ready()
            .times($times)
            .returning(|_| std::task::Poll::Ready(Ok(())));
    };
    ($name:expr) => {
        $name
            .expect_poll_ready()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Ok(())));
    };
}

pub use expect_poll_ready;
use tower::Service;

pub struct MockCloneService<S> {
    inner: Arc<Mutex<S>>,
}

impl<S> Clone for MockCloneService<S> {
    fn clone(&self) -> Self {
        MockCloneService {
            inner: self.inner.clone(),
        }
    }
}

impl<S> MockCloneService<S> {
    pub fn new(inner: S) -> MockCloneService<S> {
        MockCloneService {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

impl<Req, S> Service<Req> for MockCloneService<S>
where
    S: Service<Req> + Send + 'static,
    Req: Send + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        loop {
            if let Some(mut inner) = self.inner.try_lock() {
                return inner.poll_ready(cx);
            }
        }
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let inner = self.inner.clone();
        let fut = async move {
            let mut inner = inner.lock().await;
            inner.call(req).await
        };
        Box::pin(fut)
    }
}
