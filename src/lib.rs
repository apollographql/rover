#![cfg_attr(not(test), deny(clippy::panic,))]

pub mod cli;
pub mod command;
#[cfg(feature = "composition-js")]
pub mod composition;
mod error;
mod options;
mod subtask;
pub mod utils;
mod watch;

pub use command::RoverOutput;
pub use error::{RoverError, RoverErrorCode, RoverErrorSuggestion, RoverResult};
pub use utils::pkg::*;
