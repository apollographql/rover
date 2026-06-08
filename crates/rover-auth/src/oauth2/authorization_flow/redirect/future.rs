use std::pin::Pin;

use oauth2::{AuthorizationCode, CsrfToken};
use pin_project::pin_project;
use tokio::{
    net::TcpListener,
    sync::oneshot::{self, error::RecvError},
};
use tower::make::Shared;

use super::handler::RedirectHandler;

pub type DefaultOauthRedirectFuture = OauthRedirectFuture<
    oneshot::Receiver<String>,
    Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send>>,
>;

#[derive(thiserror::Error, Debug)]
pub enum OauthRedirectError {
    #[error("Failed to receive exchange code")]
    Receiver(RecvError),
    #[error("Redirect server shutdown unexpectedly")]
    Server(Option<std::io::Error>),
}

#[pin_project]
pub struct OauthRedirectFuture<R, S> {
    #[pin]
    receiver: R,
    #[pin]
    server: S,
}

impl
    OauthRedirectFuture<
        oneshot::Receiver<String>,
        Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send>>,
    >
{
    pub fn new(listener: TcpListener, csrf_token: CsrfToken) -> DefaultOauthRedirectFuture {
        let (sender, receiver) = oneshot::channel();
        let handler = RedirectHandler::builder()
            .csrf_token(csrf_token)
            .sender(sender.into())
            .build();
        let server = Box::pin(axum::serve(listener, Shared::new(handler)).into_future());
        OauthRedirectFuture { server, receiver }
    }
}

impl<R, S> Future for OauthRedirectFuture<R, S>
where
    R: Future<Output = Result<String, RecvError>> + Send,
    S: Future<Output = Result<(), std::io::Error>> + Send,
{
    type Output = Result<AuthorizationCode, OauthRedirectError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        if let std::task::Poll::Ready(output) = this.receiver.poll(cx) {
            return match output {
                Ok(output) => {
                    let code = AuthorizationCode::new(output);
                    std::task::Poll::Ready(Ok(code))
                }
                Err(err) => std::task::Poll::Ready(Err(OauthRedirectError::Receiver(err))),
            };
        }
        if let std::task::Poll::Ready(output) = this.server.poll(cx) {
            return std::task::Poll::Ready(Err(OauthRedirectError::Server(output.err())));
        }
        std::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        task::{Context, Waker},
    };

    use mockall::mock;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tokio::sync::oneshot::{self, error::RecvError};

    use crate::oauth2::authorization_flow::redirect::future::{
        OauthRedirectError, OauthRedirectFuture,
    };

    mock! {
        Future<T> {}
        impl<T> std::future::Future for Future<T> {
            type Output = T;
            fn poll<'a>(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'a>,
            ) -> std::task::Poll<T>;
        }
    }

    #[rstest]
    fn test_ready_from_message_received() {
        let mut mock_receiver: MockFuture<Result<String, RecvError>> = MockFuture::new();
        let mut mock_server: MockFuture<Result<(), std::io::Error>> = MockFuture::new();
        mock_receiver
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Pending);
        mock_receiver
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Ok("my_code".to_string())));
        mock_server
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Pending);
        let future = OauthRedirectFuture {
            receiver: mock_receiver,
            server: mock_server,
        };
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let result = future.as_mut().poll(&mut context);
        assert_that!(result).matches(|v| matches!(v, std::task::Poll::Pending));
        let result = future.poll(&mut context);
        assert_that!(result).matches(|v| {
            if let std::task::Poll::Ready(Ok(code)) = v {
                "my_code" == code.secret()
            } else {
                false
            }
        });
    }

    #[rstest]
    fn test_ready_from_message_receiver_error() {
        let (sender, receiver) = oneshot::channel();
        let mut mock_server: MockFuture<Result<(), std::io::Error>> = MockFuture::new();
        mock_server
            .expect_poll()
            .returning(|_| std::task::Poll::Pending);
        let future = OauthRedirectFuture {
            receiver,
            server: mock_server,
        };
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let result = future.as_mut().poll(&mut context);
        assert_that!(result).matches(|v| matches!(v, std::task::Poll::Pending));
        drop(sender);
        let result = future.poll(&mut context);
        assert_that!(result).matches(|v| {
            if let std::task::Poll::Ready(Err(err)) = v {
                err.to_string() == "Failed to receive exchange code"
            } else {
                false
            }
        });
    }

    #[rstest]
    fn test_ready_from_server_shutdown() {
        let mut mock_receiver: MockFuture<Result<String, RecvError>> = MockFuture::new();
        let mut mock_server: MockFuture<Result<(), std::io::Error>> = MockFuture::new();
        mock_receiver
            .expect_poll()
            .times(2)
            .returning(|_| std::task::Poll::Pending);
        mock_server
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Pending);
        mock_server
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Ok(())));
        let future = OauthRedirectFuture {
            receiver: mock_receiver,
            server: mock_server,
        };
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let result = future.as_mut().poll(&mut context);
        assert_that!(result).matches(|v| matches!(v, std::task::Poll::Pending));
        let result = future.poll(&mut context);
        assert_that!(result).matches(|v| {
            matches!(
                v,
                std::task::Poll::Ready(Err(OauthRedirectError::Server(None)))
            )
        });
    }

    #[rstest]
    fn test_ready_from_server_error() {
        let mut mock_receiver: MockFuture<Result<String, RecvError>> = MockFuture::new();
        let mut mock_server: MockFuture<Result<(), std::io::Error>> = MockFuture::new();
        mock_receiver
            .expect_poll()
            .times(2)
            .returning(|_| std::task::Poll::Pending);
        mock_server
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Pending);
        mock_server
            .expect_poll()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Err(std::io::Error::other("no"))));
        let future = OauthRedirectFuture {
            receiver: mock_receiver,
            server: mock_server,
        };
        let mut future = std::pin::pin!(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let result = future.as_mut().poll(&mut context);
        assert_that!(result).matches(|v| matches!(v, std::task::Poll::Pending));
        let result = future.poll(&mut context);
        assert_that!(result).matches(|v| {
            if let std::task::Poll::Ready(Err(OauthRedirectError::Server(Some(err)))) = v {
                err.to_string() == std::io::Error::other("no").to_string()
            } else {
                false
            }
        });
    }
}
