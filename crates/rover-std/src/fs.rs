use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::{fs, time::Duration};

use anyhow::{anyhow, Context};
use camino::{ReadDirUtf8, Utf8Path, Utf8PathBuf};
#[cfg(windows)]
use notify::event::{DataChange, ModifyKind};
use notify::{Config, EventKind, PollWatcher, RecursiveMode, Watcher};
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::RoverStdError;

/// The rate at which we poll files for changes
const FS_POLLING_INTERVAL: Duration = Duration::from_millis(250);

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
    pub fn watch_file(
        path: PathBuf,
        tx: UnboundedSender<Result<(), RoverStdError>>,
    ) -> CancellationToken {
        let cancellation_token = CancellationToken::new();

        let poll_watcher = PollWatcher::new(
            {
                let path = path.clone();

                move |result: Result<notify::Event, notify::Error>| {
                    // This is an early check that the file exists and that we have the right
                    // permissions for it
                    //
                    // Development note: this should only be trusted for telling us that the file
                    // either doesn't exist or that we don't have the right permissions, it shouldn't
                    // be used as the final say in whether the file _should_ exist because some
                    // platforms (eg, Windows) might keep the file around after the user has already
                    // removed it. The event handling below should be the final say by capturing events
                    // relevant to the lifecycle of a file (though, see the notes below for why we
                    // should also be cautious with that )
                    if let Err(err) = std::fs::metadata(&path) {
                        tracing::error!(
                        "When checking that {path:?} exists with the right permissions: {err:?}" //"When checking that {boxed_path} exists with the right permissions: {err:?}"
                    );
                        let _ = tx.send(Err(RoverStdError::FileRemoved {
                            file: path.display().to_string(),
                        }));
                        return;
                    }

                    let event = match result {
                        Err(err) => {
                            tracing::error!("Something went wrong watching {path:?}: {err:?}");
                            let _ = tx.send(Err(RoverStdError::FileRemoved {
                                file: path.display().to_string(),
                            }));
                            return;
                        }
                        Ok(event) => event,
                    };

                    match event.kind {
                        // For changes, Windows emits Modify(Metadata(WriteTime)); for file removals,
                        // we only get the catch-all event Modify(Data(Any)). Annoyingly, the
                        // std::fs::metadata() check above passes for windows
                        #[cfg(windows)]
                        EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                            tracing::debug!(
                                "file exists 1: {:?}",
                                fs::exists(&path).unwrap_or_default()
                            );
                            match std::fs::metadata(&path) {
                                Ok(_metadata) => {
                                    tracing::debug!(
                                        "file exists 2: {:?}",
                                        fs::exists(&path).unwrap_or_default()
                                    );
                                    if fs::exists(&path).unwrap_or_default() {
                                        tracing::debug!(
                                            "received a modify event for windows, but file exists"
                                        );
                                        let _ = tx.send(Ok(())).tap_err(|_| {
                            tracing::error!("Unable to send to filewatcher receiver because it closed. File being watched: {path:?}");
                                        });
                                    }

                                    let _ = tx.send(Err(RoverStdError::FileRemoved {
                                        file: path.display().to_string(),
                                    }));
                                    return;
                                }
                                Err(err) => {
                                    let _ = tx.send(Err(RoverStdError::FileRemoved {
                                        file: path.display().to_string(),
                                    }));
                                    return;
                                }
                            }
                        }
                        EventKind::Modify(_) => {
                            let _ = tx.send(Ok(())).tap_err(|_| {
                            tracing::error!("Unable to send to filewatcher receiver because it closed. File being watched: {path:?}");
                        });
                        }
                        unsupported_event_kind => {
                            tracing::debug!("Ignoring an unsupported event while file watching {path:?}. Unsupported event kind: {unsupported_event_kind:?}\n\nEvent: {event:?}");
                        }
                    }
                }
            },
            Config::default()
                // By polling at an interval, we get built-in debouncing
                .with_poll_interval(FS_POLLING_INTERVAL)
                // Development note: this makes polling work for pseudo filesystems like tempfs;
                // but, there is a performance cost
                .with_compare_contents(true),
        );

        let cancellation_token_c = cancellation_token.clone();

        tokio::task::spawn({
            let path = path.to_path_buf();

            async move {
                match poll_watcher {
                    Ok(mut poll_watcher) => {
                        // Internally, watch() starts a synchronous loop in a background thread
                        // that only stops when poll_watcher gets dropped
                        let _ = poll_watcher.watch(&path, RecursiveMode::NonRecursive);
                        // To keep poll_watcher from getting dropped, we wait on the cancellation
                        // token to be used. When it's used, this tokio task will end, dropping the
                        // fn, and thereby dropping the poll_watcher and ending the background
                        // thread's synchronous loop
                        cancellation_token_c.cancelled().await;
                        tracing::debug!("Dropping file watcher for: {:?}", path);
                    }
                    // If we fail to watch the file for some reason, don't panic, but let the user know
                    // that something went wrong
                    //
                    // Development note: eventually, we'll probably want to return a
                    // Result<CancellationToken, RoverStdErr> and let the caller deal with errors.
                    // Previously, we used Options to denote the existence of a watcher
                    Err(err) => {
                        tracing::error!(
                            "Something went wrong when trying to watch {path:?}: {err:?}"
                        );
                    }
                };
            }
        });

        cancellation_token
    }
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

    #[tokio::test]
    async fn test_watch_file() -> Result<()> {
        // create a temporary file that we'll make changes to for events to be watched
        let mut file = NamedTempFile::new()?;
        let path = file.path().to_path_buf();
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

        let mut writeable_file = OpenOptions::new().write(true).truncate(true).open(path)?;
        writeable_file.write_all("some change".as_bytes())?;
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

        sleep(Duration::from_millis(1000)).await;
        cancellation_token.cancel();

        sleep(Duration::from_millis(1000)).await;

        {
            let rx = rx.lock().await;
            assert_that!(rx.is_closed()).is_true();
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_shutdown_on_file_removed() -> Result<()> {
        // create a temporary file that we'll make changes to for events to be watched
        let mut file = NamedTempFile::new()?;
        let path = file.path().to_path_buf();

        let (tx, rx) = unbounded_channel();
        let rx = Arc::new(Mutex::new(rx));

        let _cancellation_token = Fs::watch_file(path.clone(), tx);

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
            .open(path.clone())?;

        writeable_file.write_all("some change".as_bytes())?;

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

        file.close()?;

        let result = tokio::time::timeout(Duration::from_millis(4000), {
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
            .is_err()
            .matches(|err| {
                // Ugly string comparison; if we ever make RoverStdError PartialEq, change this
                // (conflicts with anyhow)
                err.to_string()
                    == RoverStdError::FileRemoved {
                        file: path.display().to_string(),
                    }
                    .to_string()
            });

        Ok(())
    }
}
