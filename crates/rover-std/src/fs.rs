use anyhow::{anyhow, Context};
use camino::{ReadDirUtf8, Utf8Path, Utf8PathBuf};
use futures::{channel::mpsc, prelude::*};
use notify::{
    event::{DataChange, MetadataKind, ModifyKind},
    Config, EventKind, PollWatcher, RecursiveMode, Watcher,
};

use std::{
    fs::{self, File},
    str,
    time::Duration,
};

use crate::{Emoji, RoverStdError};

#[cfg(not(test))]
const DEFAULT_WATCH_DURATION: Duration = Duration::from_secs(3);

#[cfg(test)]
const DEFAULT_WATCH_DURATION: Duration = Duration::from_millis(100);

/// Interact with a file system
#[derive(Default, Copy, Clone)]
pub struct Fs;

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
        let contents = str::from_utf8(contents.as_ref()).with_context(|| {
            format!(
                "tried to write contents to {} that was invalid UTF-8",
                &path
            )
        })?;
        if !path.exists() {
            File::create(path)
                .with_context(|| format!("{} does not exist and it could not be created", &path))?;
        }
        if !path.exists() {
            File::create(path)
                .with_context(|| format!("{} does not exist and it could not be created", &path))?;
        }
        tracing::info!("writing {} to disk", &path);
        fs::write(path, contents).with_context(|| format!("could not write {}", &path))?;
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

    /// Creates a stream events whenever the file at the path has changes. The stream never terminates
    /// and must be dropped to finish watching.
    ///
    /// # Arguments
    ///
    /// * `path`: The file to watch
    ///
    /// returns: impl Stream<Item=()>
    ///
    // adapted from the router codebase:
    // https://github.com/apollographql/router/blob/5792c2c02d25b2998f55d7773ee71a24005b85d3/apollo-router/src/files.rs#L22C1-L30C4
    pub fn watch_file<P>(path: P) -> impl Stream<Item = ()>
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        let path_string = path.to_string();
        let std_path_buf = Utf8PathBuf::from(path).as_std_path().to_path_buf();
        let cloned_std_path_buf = std_path_buf.clone();
        eprintln!("{}watching {} for changes", Emoji::Watch, &path_string);

        let (mut watch_sender, watch_receiver) = mpsc::channel(1);
        // We can't use the recommended watcher, because there's just too much variation across
        // platforms and file systems. We use the Poll Watcher, which is implemented consistently
        // across all platforms. Less reactive than other mechanisms, but at least it's predictable
        // across all environments. We compare contents as well, which reduces false positives with
        // some additional processing burden.
        let config = Config::default()
            .with_poll_interval(DEFAULT_WATCH_DURATION)
            .with_compare_contents(true);
        let mut watcher = PollWatcher::new(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    // The two kinds of events of interest to use are writes to the metadata of a
                    // watched file and changes to the data of a watched file
                    if matches!(
                        event.kind,
                        EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime))
                            | EventKind::Modify(ModifyKind::Data(DataChange::Any))
                    ) && event.paths.contains(&std_path_buf.clone())
                    {
                        loop {
                            match watch_sender.try_send(()) {
                                Ok(_) => break,
                                Err(err) => {
                                    tracing::warn!(
                                        "could not process file watch notification. {}",
                                        err.to_string()
                                    );
                                    if err.is_full() {
                                        std::thread::sleep(Duration::from_millis(50));
                                    } else {
                                        panic!("event channel failed: {err}");
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("event error: {:?}", e),
            },
            config,
        )
        .unwrap_or_else(|_| panic!("could not watch file at path '{path_string}'"));
        watcher
            .watch(cloned_std_path_buf.as_path(), RecursiveMode::NonRecursive)
            .unwrap_or_else(|_| panic!("could not watch file at path '{path_string}'"));
        // Tell watchers once they should read the file once,
        // then listen to fs events.
        stream::once(future::ready(()))
            .chain(watch_receiver)
            .chain(stream::once(async move {
                // This exists to give the stream ownership of the hotwatcher.
                // Without it hotwatch will get dropped and the stream will terminate.
                // This code never actually gets run.
                // The ideal would be that hotwatch implements a stream and
                // therefore we don't need this hackery.
                drop(watcher);
            }))
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env::temp_dir,
        fs::File,
        io::{Seek, Write},
    };

    use camino::Utf8PathBuf;
    use test_log::test;

    use super::*;

    // this test has been copied and slightly adapted from the router codebase:
    // https://github.com/apollographql/router/blob/5792c2c02d25b2998f55d7773ee71a24005b85d3/apollo-router/src/files.rs#L102
    #[test(tokio::test)]
    async fn basic_watch() {
        let path = Utf8PathBuf::from_path_buf(temp_dir().join(format!("{}", uuid::Uuid::new_v4())))
            .unwrap();
        let mut file = std::fs::File::create(&path).unwrap();

        let mut watch = Fs::watch_file(&path);

        // This test can be very racy. Without synchronisation, all
        // we can hope is that if we wait long enough between each
        // write/flush then the future will become ready.
        assert!(futures::poll!(watch.next()).is_ready());
        write_and_flush(&mut file, "Some data 1").await;
        assert!(futures::poll!(watch.next()).is_ready());
        write_and_flush(&mut file, "Some data 2").await;
        assert!(futures::poll!(watch.next()).is_ready())
    }

    pub(crate) async fn write_and_flush(file: &mut File, contents: &str) {
        file.rewind().unwrap();
        file.set_len(0).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
