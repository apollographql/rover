use crate::Level;

use tracing_subscriber::fmt;

use std::io;

pub(crate) fn least_verbose(level: Level) {
    fmt()
        .with_max_level(level)
        .without_time()
        .with_writer(io::stderr)
        .init();
}

pub(crate) fn verbose(level: Level) {
    fmt().with_max_level(level).with_writer(io::stderr).init();
}

pub(crate) fn very_verbose(level: Level) {
    fmt()
        .with_max_level(level)
        .with_writer(io::stderr)
        .with_thread_ids(true)
        .init();
}
