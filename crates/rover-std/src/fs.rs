use std::fs::OpenOptions;
use std::io::Write;
use std::{
    fs::{self},
    path::Path,
    sync::mpsc::channel,
    time::Duration,
};

use anyhow::{anyhow, Context};
use camino::{ReadDirUtf8, Utf8Path};
use crossbeam_channel::Sender;
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use notify_debouncer_full::new_debouncer;

use crate::{Emoji, RoverStdError};

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
        if let Some(parent_path) = path.parent() {
            tracing::debug!(
                "file path contains parent path, ensuring this completely exists before continuing"
            );
            if !parent_path.exists() {
                tracing::debug!("creating parent directories");
                Self::create_dir_all(&parent_path).with_context(|| {
                    format!(
                        "{} does not exist and it could not be created",
                        &parent_path
                    )
                })?;
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .with_context(|| {
                format!(
                    "tried to write contents to {} that was invalid UTF-8",
                    &path
                )
            })?;
        tracing::info!("writing {} to disk", &path);
        file.write(contents.as_ref())
            .with_context(|| format!("could not write {}", &path))?;
        Ok(())
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
    /// let (tx, rx) = crossbeam_channel::unbounded();
    /// let path = "./test.txt";
    /// rayon::spawn(move || {
    ///   Fs::spawn_file_watcher(&path, tx)?;
    ///   rayon::spawn(move || loop {
    ///     rx.recv();
    ///     println!("file contents:\n{}", Fs::read_file(&path)?);
    ///   });
    /// });
    /// ```
    pub fn watch_file<P>(path: P, tx: WatchSender)
    where
        P: AsRef<Utf8Path>,
    {
        // Build a Rayon Thread pool
        let tp = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .thread_name(|idx| format!("file-watcher-{idx}"))
            .build()
            .expect("thread pool built successfully");
        let path = path.as_ref().to_path_buf();
        tp.spawn(move || {
            eprintln!(
                "{}watching {} for changes",
                Emoji::Watch,
                path.as_std_path().display()
            );
            let path = path.as_std_path();
            let (fs_tx, fs_rx) = channel();
            // Spawn a debouncer so we don't detect single rather than multiple writes in quick succession,
            // use the None parameter to allow it to calculate the tick_rate, in line with previous
            // notify implementations.
            let mut debouncer = match new_debouncer(Duration::from_secs(1), None, fs_tx) {
                Ok(debouncer) => debouncer,
                Err(err) => {
                    handle_notify_error(&tx, path, err);
                    return;
                }
            };
            if let Err(err) = debouncer.watcher().watch(path, RecursiveMode::NonRecursive) {
                handle_notify_error(&tx, path, err);
                return;
            }

            // Sit in the loop, and once we get an event from the file pass it along to the
            // waiting channel so that the supergraph can be re-composed.
            loop {
                let events = match fs_rx.recv() {
                    Err(err) => {
                        handle_generic_error(&tx, path, err);
                        break;
                    }
                    Ok(Err(errs)) => {
                        if let Some(err) = errs.first() {
                            handle_generic_error(&tx, path, err);
                        }
                        break;
                    }
                    Ok(Ok(events)) => events,
                };
                for event in events {
                    if let EventKind::Modify(ModifyKind::Data(..)) = event.kind {
                        if let Err(err) = tx.send(Ok(())) {
                            handle_generic_error(&tx, path, err);
                            break;
                        }
                    }
                }
            }
        })
    }
}

type WatchSender = Sender<Result<(), RoverStdError>>;

/// User-friendly error messages for `notify::Error` in `watch_file`
fn handle_notify_error(tx: &WatchSender, path: &Path, err: notify::Error) {
    match &err.kind {
        notify::ErrorKind::PathNotFound => eprintln!(
            "{} could not watch \"{}\" for changes: file not found",
            Emoji::Warn,
            path.display()
        ),
        notify::ErrorKind::MaxFilesWatch => {
            eprintln!(
                "{} could not watch \"{}\" for changes: total number of inotify watches reached, consider increasing the number of allowed inotify watches or stopping processed that watch many files",
                Emoji::Warn,
                path.display()
            );
        }
        notify::ErrorKind::Generic(_)
        | notify::ErrorKind::Io(_)
        | notify::ErrorKind::WatchNotFound
        | notify::ErrorKind::InvalidConfig(_) => eprintln!(
            "{} an unexpected error occured while watching {} for changes",
            Emoji::Warn,
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
