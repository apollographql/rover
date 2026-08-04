mod compose;
/// Only builds when native composition is compiled in; release artifacts do not enable it.
#[cfg(feature = "composition-rust")]
mod composition_parity;
mod config;
mod fetch;
