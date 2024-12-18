//! A Subtask is a task that runs in the background and is able to receive events and (if the
//! SubtaskHandleStream trait is implemented) emit events via unbounded channels.
//!
//! There are two important traits that you'll implement in order to use Subtasks:
//!
//!   SubtaskHandleUnit - for receiving events,
//!   SubtaskHandleStream - for both receiving and emitting events
//!
//! There are examples in the codebase for both, but they follow a similar pattern: a `handle`
//! function is implemented that receives an `UnboundedSender<Self::Output>` for some `Output`
//! defined by the consumer. For `SubtaskHandleStream`, an `input` is required for receiving
//! events
//!
//!
//! Here is an example implementation of implementing `SubtaskHandleUnit`
//!
//! ```rust,ignore
//!  impl SubtaskHandleUnit for SomeType {
//!      // maybe this exists above, maybe it's something simple like an
//!      //empty tuple
//!      type Output = SomeOutput;
//!
//!      fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
//!          tokio::spawn(async move {
//!              // Make sure you keep the watcher's from being called multiple times by
//!              // putting it on its own line
//!              let mut watcher = self.some_fn_for_recv_events().await;
//!              // Watching for events is pretty straightforward when you have a BoxStream, which
//!              // gives you .next()
//!              while let Some(_change) = watcher.next().await {
//!                  // When something happens that we want to react to, we can emit an event
//!                  let _ = sender
//!                      .send(SomeType)
//!                      .tap_err(|err| tracing::error!("{:?}", err));
//!              }
//!          })
//!          // An abort handle is returned in case we need to abort the task (eg, for some failure
//!          // outside of it)
//!          .abort_handle()
//!      }
//!  }
//! ```
//!
//! Once you've implemented either SubtaskHandleUnit or SubtaskHandleStream for a type, you can
//! `.run()` it to begin the Subtask and receive an UnboundedStream for sending the events being
//! emitted from that Subtask to other consumers:
//!
//! ```rust,ignore
//!  // Create the SomeType Subtask, returning a receiver for others to stream events coming out
//!  // of the SomeType subtask
//!  let (events_for_others_to_ingest, sometype_subtask) = Subtask::new(SomeType);
//!
//!  // Listen to events coming from some other Subtask
//!  sometype_subtask.run(some_other_event_stream);
//! ```

use futures::stream::BoxStream;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::AbortHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

/// A trait whose implementation will be able to send events
pub trait SubtaskHandleUnit {
    type Output;
    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle;
}

/// A trait whose implementation will be able to both send and receive events
pub trait SubtaskHandleStream {
    type Input;
    type Output;
    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle;
}

/// A trait whose implementation can run a subtask that only ingests messages
pub trait SubtaskRunUnit {
    fn run(self) -> AbortHandle;
}

/// A trait whose implementation can run a subtask that can both ingest messages and emit them
pub trait SubtaskRunStream {
    type Input;
    fn run(self, input: BoxStream<'static, Self::Input>) -> AbortHandle;
}

/// A background task that can emit messages via a sender channel
#[derive(Debug)]
pub struct Subtask<T, Output> {
    inner: T,
    sender: UnboundedSender<Output>,
}

impl<T, Output> Subtask<T, Output> {
    /// Crates a new Subtask with unbounded channels for transmitting and receiving. The
    /// transmitter is returned to the caller so that it can be used to send messages to the
    /// Subtask's receiver
    pub fn new(inner: T) -> (UnboundedReceiverStream<Output>, Subtask<T, Output>) {
        let (tx, rx) = unbounded_channel();
        (
            UnboundedReceiverStream::new(rx),
            Subtask { inner, sender: tx },
        )
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: SubtaskHandleUnit<Output = Output>, Output> SubtaskRunUnit for Subtask<T, Output> {
    /// Begin running the subtask, calling handle() on the type implementing the SubTaskHandleUnit trait
    fn run(self) -> AbortHandle {
        self.inner.handle(self.sender)
    }
}

impl<T: SubtaskHandleStream<Output = Output>, Output> SubtaskRunStream for Subtask<T, Output> {
    type Input = T::Input;

    /// Begin running the subtask with a stream of events, calling handle() on the type implementing the SubTaskHandleStream trait
    fn run(self, input: BoxStream<'static, Self::Input>) -> AbortHandle {
        self.inner.handle(self.sender, input)
    }
}
