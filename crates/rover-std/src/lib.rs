#![cfg_attr(not(test), deny(clippy::panic,))]

mod error;
mod fs;
mod style;
pub mod url;

pub mod print;
pub mod prompt;
pub use error::RoverStdError;
pub use fs::Fs;
pub use style::is_no_color_set;
pub use style::Style;
pub use url::hyperlink;
pub use url::sanitize_url;
