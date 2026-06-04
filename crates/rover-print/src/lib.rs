#[cfg(feature = "cli")]
pub mod cli;
pub mod print;
pub mod style;

pub use print::{stderr, stdout};
