use super::supergraph::binary::{CompositionError, CompositionOutput};
use camino::Utf8PathBuf;
use tokio::sync::broadcast::{
    self,
    error::{RecvError, SendError},
    Receiver, Sender,
};

//// TODO: move to dev
//#[derive(Clone)]
//enum DevEvent {
//    Composition(CompositionEvent),
//    Watcher(WatcherEvent),
//}

#[derive(Clone)]
pub enum CompositionEvent {
    Started,
    Success(CompositionOutput),
    Error(CompositionError),
}

//#[derive(Clone)]
//enum WatcherEvent {
//    Opened(Watcher),
//    Updated(Watcher),
//    Closed(Watcher),
//}

// TODO: replace or amend with whatever watcher stuct we end up with in dev
//#[derive(Clone)]
//struct Watcher {
//    name: String,
//    source: WatcherSource,
//    // TODO: this needs to be something other than compositionoutput, according to the doc; not
//    // sure yet (asked), but they want changes from updates
//    change: Option<Vec<CompositionOutput>>,
//}
//
//// NB: schema source might come from fed-rs
//#[derive(Clone)]
//enum WatcherSource {
//    GraphRef(String),
//    Introspection,
//    // TODO: wtf does this mean?
//    Inline,
//    File(Utf8PathBuf),
//}
//
//enum EventRunnerError {
//    /// Receivers all closed, but we're able to resubscribe
//    NoReceivers(DevEvent),
//    NoSenders,
//    /// The receiver of the events is lagging and can now only retreive stale messages
//    StaleDelay(u64),
//}
//
//// watch for multi-producer, multi-consumer; we'll want the lsp to be a consumer, but also have
//// consumers for dev (and maybe composition? not sure)
//struct EventRunner {
//    receiver: Receiver<DevEvent>,
//    sender: Sender<DevEvent>,
//}
//
//impl EventRunner {
//    fn new() -> Self {
//        // TODO decide on capacity
//        let (tx, rx) = broadcast::channel(100);
//
//        Self {
//            sender: tx,
//            receiver: rx,
//        }
//    }
//
//    async fn rec(&mut self) -> Result<DevEvent, EventRunnerError> {
//        Ok(self.receiver.recv().await?)
//    }
//
//    async fn emit(&self, event: DevEvent) -> Result<(), EventRunnerError> {
//        self.sender.send(event)?;
//        Ok(())
//    }
//}
//
//impl From<SendError<DevEvent>> for EventRunnerError {
//    fn from(value: SendError<DevEvent>) -> Self {
//        match value {
//            SendError(event) => EventRunnerError::NoReceivers(event),
//        }
//    }
//}
//
//impl From<RecvError> for EventRunnerError {
//    fn from(value: RecvError) -> Self {
//        match value {
//            RecvError::Closed => EventRunnerError::NoSenders,
//            RecvError::Lagged(events_skipped) => EventRunnerError::StaleDelay(events_skipped),
//        }
//    }
//}
