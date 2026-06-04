use std::fmt;

use crate::style::StyledText;

#[cfg_attr(any(test, feature = "testing"), mockall::automock)]
pub trait Print {
    fn print(&self, message: &StyledText) -> std::io::Result<()>;
}

/// Printing for Humans
#[derive(Clone, Debug, bon::Builder)]
pub struct Term {
    term: console::Term,
    with_color: bool,
}

impl Print for Term {
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        if self.with_color {
            self.term.write_line(&message.paint())
        } else {
            self.term.write_line(message.text())
        }
    }
}

pub struct Stderr<P>
where
    P: Print,
{
    print: P,
}

// Implemented separately to avoid requiring type constraints throughout consumer code
impl<P> fmt::Debug for Stderr<P>
where
    P: Print + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Stderr")
            .field("print", &self.print)
            .finish()
    }
}

impl Stderr<Term> {
    pub fn term(with_color: bool) -> Stderr<Term> {
        Stderr {
            print: Term {
                term: console::Term::stderr(),
                with_color,
            },
        }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Stderr<MockPrint> {
    pub fn mock() -> Stderr<MockPrint> {
        Stderr {
            print: MockPrint::new(),
        }
    }
}

impl Default for Stderr<Term> {
    fn default() -> Self {
        Self::term(true)
    }
}

impl<P> Print for Stderr<P>
where
    P: Print,
{
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        self.print.print(message)
    }
}

pub struct Stdout<P>
where
    P: Print,
{
    print: P,
}

// Implemented separately to avoid requiring type constraints throughout consumer code
impl<P> fmt::Debug for Stdout<P>
where
    P: Print + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Stdout")
            .field("print", &self.print)
            .finish()
    }
}

impl Stdout<Term> {
    pub fn term(with_color: bool) -> Stdout<Term> {
        Stdout {
            print: Term {
                term: console::Term::stdout(),
                with_color,
            },
        }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Stdout<MockPrint> {
    pub fn mock() -> Stdout<MockPrint> {
        Stdout {
            print: MockPrint::new(),
        }
    }
}

impl Default for Stdout<Term> {
    fn default() -> Self {
        Self::term(true)
    }
}

impl<P> Print for Stdout<P>
where
    P: Print,
{
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        self.print.print(message)
    }
}
