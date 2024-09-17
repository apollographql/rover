// TODO: we should document this

use futures::stream::BoxStream;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::AbortHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub trait SubtaskRunUnit {
    fn run(self) -> AbortHandle;
}

pub trait SubtaskRunStream {
    type Input;
    fn run(self, input: BoxStream<'static, Self::Input>) -> AbortHandle;
}

pub trait SubtaskHandleUnit {
    type Output;
    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle;
}

pub trait SubtaskHandleStream {
    type Input;
    type Output;
    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle;
}

pub struct Subtask<T, Output> {
    inner: T,
    sender: UnboundedSender<Output>,
}

impl<T, Output> Subtask<T, Output> {
    pub fn new(inner: T) -> (UnboundedReceiverStream<Output>, Subtask<T, Output>) {
        let (tx, rx) = unbounded_channel();
        (
            UnboundedReceiverStream::new(rx),
            Subtask { inner, sender: tx },
        )
    }
}

impl<T: SubtaskHandleUnit<Output = Output>, Output> SubtaskRunUnit for Subtask<T, Output> {
    fn run(self) -> AbortHandle {
        self.inner.handle(self.sender)
    }
}

impl<T: SubtaskHandleStream<Output = Output>, Output> SubtaskRunStream for Subtask<T, Output> {
    type Input = T::Input;
    fn run(self, input: BoxStream<'static, Self::Input>) -> AbortHandle {
        self.inner.handle(self.sender, input)
    }
}
