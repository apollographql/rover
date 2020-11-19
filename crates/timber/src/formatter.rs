use crate::Level;

use tracing_subscriber::fmt;

use std::io;

pub(crate) fn least_verbose(level: Level) {
    let format = fmt::format().without_time().with_target(false).compact();
    fmt()
        .with_max_level(level)
        .event_format(format)
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
