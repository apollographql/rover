use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::{fs, path::Path, time::Duration};

use anyhow::{anyhow, Context};
use camino::{ReadDirUtf8, Utf8Path, Utf8PathBuf};
use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use tap::TapFallible;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{Receiver, Sender as BoundedSender, UnboundedSender};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{errln, infoln, warnln, RoverStdError};

/// The rate at which we timeout the debouncer
const DEBOUNCER_TIMEOUT: Duration = Duration::from_secs(1);

/// Interact with a file system
#[derive(Default, Copy, Clone)]
pub struct Fs {}

impl Fs {
    /// reads a file from disk
    pub fn read_file<P>(path: P) -> Result<String, RoverStdError>
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        match fs::metadata(path) {
            Ok(metadata) => {
                if metadata.is_file() {
                    tracing::info!("reading {} from disk", &path);
                    let contents = fs::read_to_string(path)
                        .with_context(|| format!("could not read {}", &path))?;
                    if contents.is_empty() {
                        Err(RoverStdError::EmptyFile {
                            empty_file: path.to_string(),
                        })
                    } else {
                        Ok(contents)
                    }
                } else {
                    Err(anyhow!("'{}' is not a file", path).into())
                }
            }
            Err(e) => Err(anyhow!("could not find '{}'", path).context(e).into()),
        }
    }

    /// writes a file to disk
    pub fn write_file<P, C>(path: P, contents: C) -> Result<(), RoverStdError>
    where
        P: AsRef<Utf8Path>,
        C: AsRef<[u8]>,
    {
        let path = path.as_ref();
        tracing::info!("checking existence of parent path in '{}'", path);

        // Try and grab the last element of the path, which should be the file name, if we can't
        // then we should bail out and throw that back to the user.
        let file_name = path.file_name().ok_or(anyhow!(
            "cannot write to a path without a final element {path}"
        ))?;

        // Grab the parent path then attempt to canonicalize it, we can't just canonicalize the
        // entire path because that would entail the file existing, which of course it doesn't yet.
        let mut canonical_final_path = path
            .parent()
            .map(Self::upsert_path_exists)
            .ok_or(anyhow!("cannot write file to root or prefix {path}"))??;

        // Create the final version of the path we want to create
        canonical_final_path.push(file_name);

        tracing::debug!("final canonical path is {}", canonical_final_path);
        // Setup a file pointer
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| {
                format!(
                    "tried to open {} but was unable to do so",
                    &canonical_final_path
                )
            })?;
        tracing::info!("writing {} to disk", &canonical_final_path);
        // Actually write the file out to where it needs to be
        file.write(contents.as_ref())
            .with_context(|| format!("could not write {}", &canonical_final_path))?;
        Ok(())
    }

    /// Given a path, where some elements may not exist, it will return the canonical
    /// representation of the path, AND create any missing interim directories.
    fn upsert_path_exists(path: &Utf8Path) -> Result<Utf8PathBuf, anyhow::Error> {
        tracing::debug!("attempting to canonicalize parent path '{path}'");
        if let Err(e) = path.canonicalize_utf8() {
            match e.kind() {
                ErrorKind::NotFound => {
                    tracing::debug!("could not canonicalize parent path '{}', attempting to create interim paths", path);
                    // If the canonicalization fails, then some part of the chain must not exist,
                    // so we need to call create_dir_all to fix this
                    Self::create_dir_all(path).with_context(|| {
                        format!("{} does not exist and it could not be created", &path)
                    })?;
                    tracing::debug!("interim paths created for {}", path);
                }
                ErrorKind::PermissionDenied => {
                    return Err(anyhow::anyhow!(
                        "cannot write file to path {} as user does not have permissions to do so",
                        path
                    ))
                }
                _ => {}
            }
        }
        path.canonicalize_utf8().map_err(|e| anyhow!(e))
    }

    /// creates a directory
    pub fn create_dir_all<P>(path: P) -> Result<(), RoverStdError>
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        tracing::info!("creating {} directory", &path);
        fs::create_dir_all(path)
            .with_context(|| format!("could not create {} directory", &path))?;
        Ok(())
    }

    /// get contents of a directory
    pub fn get_dir_entries<D>(dir: D) -> Result<ReadDirUtf8, RoverStdError>
    where
        D: AsRef<Utf8Path>,
    {
        let dir = dir.as_ref();
        let entries = dir
            .read_dir_utf8()
            .with_context(|| format!("could not read entries of {}", dir))?;
        Ok(entries)
    }

    /// assert that a file exists
    pub fn assert_path_exists<F>(file: F) -> Result<(), RoverStdError>
    where
        F: AsRef<Utf8Path>,
    {
        let file = file.as_ref();
        Self::metadata(file)?;
        Ok(())
    }

    /// get metadata about a file path
    pub fn metadata<F>(file: F) -> Result<fs::Metadata, RoverStdError>
    where
        F: AsRef<Utf8Path>,
    {
        let file = file.as_ref();
        Ok(fs::metadata(file)
            .with_context(|| format!("could not find a file at the path '{}'", file))?)
    }

    /// copies one file to another
    pub fn copy<I, O>(in_path: I, out_path: O) -> Result<(), RoverStdError>
    where
        I: AsRef<Utf8Path>,
        O: AsRef<Utf8Path>,
    {
        let in_path = in_path.as_ref();
        let out_path = out_path.as_ref();
        tracing::info!("copying {} to {}", in_path, out_path);
        // attempt to remove the old file
        // but do not error if it doesn't exist.
        let _ = fs::remove_file(out_path);
        fs::copy(in_path, out_path)
            .with_context(|| format!("could not copy {} to {}", &in_path, &out_path))?;
        Ok(())
    }

    /// recursively removes directories
    pub fn remove_dir_all<D>(dir: D) -> Result<(), RoverStdError>
    where
        D: AsRef<Utf8Path>,
    {
        let dir = dir.as_ref();
        if Self::path_is_dir(dir)? {
            fs::remove_dir_all(dir).with_context(|| format!("could not remove {}", dir))?;
            Ok(())
        } else {
            Err(anyhow!("could not remove {} because it is not a directory", dir).into())
        }
    }

    /// checks if a path is a directory, errors if the path does not exist
    pub fn path_is_dir<D>(dir: D) -> Result<bool, RoverStdError>
    where
        D: AsRef<Utf8Path>,
    {
        let dir = dir.as_ref();
        Self::metadata(dir).map(|m| m.is_dir())
    }

    /// copies all contents from one directory to another
    pub fn copy_dir_all<I, O>(in_dir: I, out_dir: O) -> Result<(), RoverStdError>
    where
        I: AsRef<Utf8Path>,
        O: AsRef<Utf8Path>,
    {
        let in_dir = in_dir.as_ref();
        let out_dir = out_dir.as_ref();
        Self::create_dir_all(out_dir)?;
        tracing::info!("copying contents of {} to {}", in_dir, out_dir);
        for entry in (Self::get_dir_entries(in_dir)?).flatten() {
            let entry_path = entry.path();
            if let Ok(metadata) = fs::metadata(entry_path) {
                if metadata.is_file() {
                    if let Some(entry_name) = entry_path.file_name() {
                        let out_file = out_dir.join(entry_name);
                        tracing::info!("copying {} to {}", &entry_path, &out_file);
                        fs::copy(entry_path, &out_file).with_context(|| {
                            format!("could not copy {} to {}", &entry_path, &out_file)
                        })?;
                    }
                } else if metadata.is_dir() && entry_path != in_dir {
                    if let Some(entry_name) = entry_path.file_name() {
                        let out_dir = out_dir.join(entry_name);
                        tracing::info!("copying {} to {}", &entry_path, &out_dir);
                        Fs::copy_dir_all(entry_path, &out_dir)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Spawns a file watcher for a given file, sending events over the channel whenever the file
    /// should be re-read. This is primarily used for composition and so the event emitted is a
    /// unit struct. The caller should react to that event as representing a reason to recompose.
    ///
    /// Example:
    ///
    /// ```ignore
    /// let path = "./test.txt";
    /// let (file_tx, file_rx) = tokio::sync::mypsc::unbounded_channel();
    /// let cancellation_token = Fs::watch_file(path.clone(), file_tx);
    ///
    /// tokio::spawn(move || {
    ///     while let Some(event) = file_rx.await {
    ///         // do something
    ///     }
    /// });
    ///
    /// // Cancel and close the watcher
    /// cancellation_token.cancel();
    /// ```
    pub fn watch_file<P>(path: P, tx: WatchSender) -> CancellationToken
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref().to_path_buf().into_std_path_buf();
        let (fs_tx, fs_rx) = tokio::sync::mpsc::channel::<DebounceEventResult>(1);

        infoln!("Watching {:?} for changes", path.display());

        let runtime_handle = Handle::current();
        let debouncer = Fs::debouncer(&runtime_handle, &tx, fs_tx, path.clone());
        let receive_messages_join_handle = Fs::receive_messages(tx, fs_rx, path);
        let cancellation_token = CancellationToken::new();

        tokio::spawn({
            let debouncer = debouncer;
            let messages_abort_handle = receive_messages_join_handle.abort_handle();
            let cancellation_token = cancellation_token.clone();

            async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::debug!("file watching cancelled");
                        if let Some(debouncer) = debouncer {
                            drop(debouncer);
                        }
                        messages_abort_handle.abort();
                    }
                    _ = async move {
                        tokio::join!(receive_messages_join_handle)
                    } => {}
                }
            }
        });

        cancellation_token
    }

    /// Spawns a debouncer for use in keeping multiple, successive writes from having to be
    /// processed. Rather, events are emitted at the timeout rate and are checked for at particular
    /// intervals (see the documentation on `new_debouncer`)
    ///
    /// Returns an option to denote whether we're successfully watching with a debouncer; if not,
    /// None is returned
    ///
    /// Development note: the RecommendedWatcher is platform-specific and _might_ be a good place
    /// for debugging if you run into weird behavior for the deboucner's watcher
    fn debouncer(
        runtime_handle: &Handle,
        watching_tx: &WatchSender,
        fs_tx: BoundedSender<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
        path: PathBuf,
    ) -> Option<Debouncer<RecommendedWatcher, FileIdMap>> {
        let path = path.as_path();
        let runtime_handle = runtime_handle.clone();

        let err_notification = |err: notify::Error| {
            handle_notify_error(watching_tx, path, err);
        };

        // The 'guts' of the debouncer and how it sends file system events
        let event_handler = move |result: DebounceEventResult| {
            runtime_handle.block_on(async {
                let _ = fs_tx
                    .send(result)
                    .await
                    .tap_err(|err| warnln!("Failed to send DebounceEventResult: {:?}", err));
            });
        };

        // Create a new debouncer
        new_debouncer(
            DEBOUNCER_TIMEOUT,
            // The tick rate; when None, notify caltures it for us (1/4th the provided timeout)
            None,
            event_handler,
        )
        .map(|mut debouncer| {
            debouncer
                .watcher()
                // Actually begin watching, but with the debouncer; non-recursive because we care
                // only about the particular file we're targeting
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(err_notification)
                .map_or(None, |_| Some(debouncer))
        })
        .map_err(err_notification)
        .unwrap_or_default()
    }

    /// Receive the file system events for ap articular file. The events emitted by particular OSes
    /// can differ, with the default Any from notify being the catch-all. See the notes within this
    /// function's body for more details on those OS-specific events
    fn receive_messages(
        watching_tx: WatchSender,
        mut fs_rx: Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
        path: PathBuf,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(events) = fs_rx.recv().await {
                let events = match events {
                    Err(errs) => {
                        if let Some(err) = errs.first() {
                            handle_generic_error(&watching_tx, path.as_path(), err);
                        }
                        break;
                    }
                    Ok(events) => events,
                };

                for event in events {
                    match event.kind {
                        // On unix-based systems, the Modify(Data(..)) tells us that the file was
                        // modified, but on windows, we have to look for the catch-all event (Any)
                        // to know whether the file was modified. Strictly speaking, we only need
                        // to match on Modify(_), but having both here should serve as a reminder
                        // to future maintainers that file system events need special care for
                        // windows
                        EventKind::Modify(ModifyKind::Data(..))
                        | EventKind::Modify(ModifyKind::Any) => {
                            if let Err(err) = watching_tx.send(Ok(())) {
                                handle_generic_error(&watching_tx, &path, err);
                                break;
                            }
                        }
                        unsupported_event_kind => {
                            tracing::debug!("encountered an unsupported event while file watching {path:?}, {unsupported_event_kind:?}: {event:?}");
                        }
                    }
                }
            }
        })
    }
}

type WatchSender = UnboundedSender<Result<(), RoverStdError>>;

/// User-friendly error messages for `notify::Error` in `watch_file`
fn handle_notify_error(tx: &WatchSender, path: &Path, err: notify::Error) {
    match &err.kind {
        notify::ErrorKind::PathNotFound => errln!(
            "could not watch \"{}\" for changes: file not found",
            path.display()
        ),
        notify::ErrorKind::MaxFilesWatch => {
            errln!(
                "could not watch \"{}\" for changes: total number of inotify watches reached, consider increasing the number of allowed inotify watches or stopping processed that watch many files",
                path.display()
            );
        }
        notify::ErrorKind::Generic(_)
        | notify::ErrorKind::Io(_)
        | notify::ErrorKind::WatchNotFound
        | notify::ErrorKind::InvalidConfig(_) => errln!(
            "an unexpected error occured while watching {} for changes",
            path.display()
        ),
    }

    tracing::debug!(
        "an unexpected error occured while watching {} for changes: {err:?}",
        path.display()
    );

    tx.send(Err(err.into())).ok();
}

/// User-friendly error messages for errors in watch_file
fn handle_generic_error<E: std::error::Error>(tx: &WatchSender, path: &Path, err: E) {
    tracing::debug!(
        "an unexpected error occured while watching {} for changes: {err:?}",
        path.display()
    );

    tx.send(Err(anyhow!(
        "an unexpected error occured while watching {} for changes: {err:?}",
        path.display()
    )
    .into()))
        .ok();
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tempfile::{NamedTempFile, TempDir};
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::sync::Mutex;
    use tokio::time::sleep;

    use super::*;

    #[rstest]
    #[case("a/b/c", "a/b/c/supergraph.yaml", vec!(), false)]
    #[case("a/b", "a/b/c/supergraph.yaml", vec!(), false)]
    #[case("/", "supergraph.yaml", vec!(), false)]
    #[case("/", "/", vec!(), true)]
    #[case("/", "abc/def", vec!("abc".to_string()), true)]
    #[case("abc", "abc/def/pqr", vec!("abc/def".to_string()), true)]
    #[case("/", "abc/../abc/def/supergraph.yaml", vec!(), false)]
    fn test_write_file(
        #[case] existing_path: &str,
        #[case] path_to_create: &str,
        #[case] existing_files: Vec<String>,
        #[case] error_expected: bool,
    ) {
        // Set up a temporary directory as required
        let bounding_dir = TempDir::new()
            .expect("failed to create temporary directory")
            .into_path();
        let mut path =
            Utf8PathBuf::from_path_buf(bounding_dir.clone()).expect("could not create UTF8-Path");
        let mut expected_path = path.clone();
        path.push(existing_path);
        // Create all the existing directories
        fs::create_dir_all(path).expect("could not set up test conditions");
        // Create any pre-existing files
        for file_to_create in existing_files.iter() {
            let mut file_to_write = Utf8PathBuf::from_path_buf(bounding_dir.clone())
                .expect("could not create UTF8-Path to file to create");
            file_to_write.push(file_to_create);
            fs::write(file_to_write, "blah, blah, blah").expect("could not write to file");
        }

        // Invoke the method to create the files
        expected_path.push(path_to_create);
        let res = Fs::write_file(expected_path.clone(), "foo, bar, bash");
        // Run assertions on the result
        if error_expected {
            assert_that(&res).is_err();
        } else {
            assert_that(&res).is_ok();
            assert_that(&expected_path).exists()
        }
    }

    //#[tokio::test]
    //async fn test_watch_file() -> Result<()> {
    //    // create a temporary file that we'll make changes to for events to be watched
    //    let mut file = NamedTempFile::new()?;
    //    let path = Utf8PathBuf::from_path_buf(file.path().to_path_buf())
    //        .unwrap_or_else(|path| panic!("Unable to create Utf8PathBuf from path: {:?}", path));

    //    let (tx, rx) = unbounded_channel();
    //    let rx = Arc::new(Mutex::new(rx));
    //    let cancellation_token = Fs::watch_file(path.clone(), tx);
    //    sleep(Duration::from_millis(1500)).await;
    //    {
    //        let rx = rx.lock().await;
    //        assert_that!(rx.is_empty()).is_true();
    //    }
    //    file.write_all(b"some update")?;
    //    file.flush()?;
    //    let result = tokio::time::timeout(Duration::from_millis(2000), {
    //        let rx = rx.clone();
    //        async move {
    //            let mut output = None;
    //            let mut rx = rx.lock().await;
    //            if let Some(message) = rx.recv().await {
    //                output = Some(message);
    //            }
    //            output
    //        }
    //    })
    //    .await;
    //    assert_that!(result)
    //        .is_ok()
    //        .is_some()
    //        .is_ok()
    //        .is_equal_to(());
    //    {
    //        let rx = rx.lock().await;
    //        assert_that!(rx.is_closed()).is_false();
    //    }
    //    cancellation_token.cancel();
    //    // Kick the event loop so that the cancellation future gets called
    //    sleep(Duration::from_millis(0)).await;
    //    {
    //        let rx = rx.lock().await;
    //        assert_that!(rx.is_closed()).is_true();
    //    }
    //    Ok(())
    //}

    #[tokio::test]
    async fn test_watch_file() -> Result<()> {
        // create a temporary file that we'll make changes to for events to be watched
        let mut file = NamedTempFile::new()?;

        let path = Utf8PathBuf::from_path_buf(file.path().to_path_buf())
            .unwrap_or_else(|path| panic!("Unable to create Utf8PathBuf from path: {:?}", path));

        let (tx, rx) = unbounded_channel();
        let rx = Arc::new(Mutex::new(rx));

        let cancellation_token = Fs::watch_file(path.clone(), tx);

        sleep(Duration::from_millis(1500)).await;

        // assert that no events have been emitted yet
        {
            let rx = rx.lock().await;
            assert_that!(rx.is_empty()).is_true();
        }

        // do a change that'll emit an event
        file.write_all(b"some update")?;
        file.flush()?;

        let mut writeable_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .expect("Cannot open file");

        writeable_file
            .write("some change".as_bytes())
            .expect("couldn't write to file");

        let result = tokio::time::timeout(Duration::from_millis(2000), {
            let rx = rx.clone();
            async move {
                let mut output = None;
                let mut rx = rx.lock().await;
                if let Some(message) = rx.recv().await {
                    output = Some(message);
                }
                output
            }
        })
        .await;

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_ok()
            .is_equal_to(());

        {
            let rx = rx.lock().await;
            assert_that!(rx.is_closed()).is_false();
        }
        cancellation_token.cancel();
        // Kick the event loop so that the cancellation future gets called
        sleep(Duration::from_millis(0)).await;
        {
            let rx = rx.lock().await;
            assert_that!(rx.is_closed()).is_true();
        }
        Ok(())
    }
}
