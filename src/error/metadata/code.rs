use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Code {}

impl Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &self)
    }
}
