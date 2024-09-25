use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::{fs, path::Path, time::Duration};

use anyhow::{anyhow, Context};
use camino::{ReadDirUtf8, Utf8Path, Utf8PathBuf};
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tap::TapFallible;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::{errln, infoln, warnln, RoverStdError};

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

    /// Spawns a file watcher for a given file, sending events over the channel
    /// whenever the file should be re-read
    ///
    /// Example:
    ///
    /// ```ignore
    /// let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    /// let path = "./test.txt";
    /// tokio::spawn(move || {
    ///   Fs::spawn_file_watcher(&path, tx)?;
    ///   tokio::task::spawn_blocking(move || loop {
    ///     rx.recv().await;
    ///     println!("file contents:\n{}", Fs::read_file(&path)?);
    ///   });
    /// });
    /// ```
    pub fn watch_file<P>(path: P, tx: WatchSender) -> CancellationToken
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref().to_path_buf();
        let path = path.as_std_path().to_path_buf();
        infoln!("Watching {} for changes", path.display());
        let (fs_tx, mut fs_rx) = tokio::sync::mpsc::channel::<DebounceEventResult>(1);

        // Sit in the loop, and once we get an event from the file pass it along to the
        // waiting channel so that the supergraph can be re-composed.
        let tx = tx.clone();
        let path = path.clone();
        let handle = Handle::current();
        // Spawn a debouncer so we don't detect single rather than multiple writes in quick succession,
        // use the None parameter to allow it to calculate the tick_rate, in line with previous
        // notify implementations.
        let debouncer = new_debouncer(
            Duration::from_secs(1),
            None,
            move |result: DebounceEventResult| {
                handle.block_on(async {
                    let _ = fs_tx
                        .send(result)
                        .await
                        .tap_err(|err| warnln!("Failed to send DebounceEventResult: {:?}", err));
                });
            },
        );

        let debouncer = match debouncer {
            Ok(mut debouncer) => {
                let watch_result = debouncer
                    .watcher()
                    .watch(&path, RecursiveMode::NonRecursive);
                match watch_result {
                    Ok(_) => Some(debouncer),
                    Err(err) => {
                        handle_notify_error(&tx, &path, err);
                        None
                    }
                }
            }
            Err(err) => {
                handle_notify_error(&tx, &path, err);
                None
            }
        };

        let receive_messages = tokio::spawn(async move {
            while let Some(events) = fs_rx.recv().await {
                let events = match events {
                    Err(errs) => {
                        if let Some(err) = errs.first() {
                            handle_generic_error(&tx, &path, err);
                        }
                        break;
                    }
                    Ok(events) => events,
                };
                for event in events {
                    if let EventKind::Modify(ModifyKind::Data(..)) = event.kind {
                        if let Err(err) = tx.send(Ok(())) {
                            handle_generic_error(&tx, &path, err);
                            break;
                        }
                    }
                }
            }
        });

        let cancellation_token = CancellationToken::new();
        tokio::spawn({
            let debouncer = debouncer;
            let cancellation_token = cancellation_token.clone();
            let messages_abort_handle = receive_messages.abort_handle();
            async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        if let Some(debouncer) = debouncer {
                            drop(debouncer);
                        }
                        messages_abort_handle.abort();
                    }
                    _ = async move {
                        tokio::join!(receive_messages)
                    } => {}
                }
            }
        });
        cancellation_token
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

    #[tokio::test]
    async fn test_watch_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let path = Utf8PathBuf::from_path_buf(file.path().to_path_buf())
            .unwrap_or_else(|path| panic!("Unable to create Utf8PathBuf from path: {:?}", path));
        let (tx, rx) = unbounded_channel();
        let rx = Arc::new(Mutex::new(rx));
        let cancellation_token = Fs::watch_file(&path, tx);
        sleep(Duration::from_millis(1500)).await;
        {
            let rx = rx.lock().await;
            assert_that!(rx.is_empty()).is_true();
        }
        file.write_all(b"test")?;
        file.flush()?;
        let result = tokio::time::timeout(Duration::from_millis(1500), {
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
