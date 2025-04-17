/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that an info prefix is
/// printed before the message.
#[macro_export]
macro_rules! infoln {
    ($($t:tt)*) => {{
        eprint!("{} ", $crate::Style::InfoPrefix.paint("==>"));
        eprintln!($($t)*);
    }};
}
/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that an info prefix is
/// printed before the message.
#[macro_export]
macro_rules! debugln {
    ($($t:tt)*) => {{
        eprint!("{} ", $crate::Style::DebugPrefix.paint("debug:"));
        eprintln!($($t)*);
    }};
}

/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that a warning prefix is
/// printed before the message.
#[macro_export]
macro_rules! warnln {
    ($($t:tt)*) => {{
        eprint!("{} ", $crate::Style::WarningPrefix.paint("warning:"));
        eprintln!($($t)*);
    }};
}

/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that an error prefix is
/// printed before the message.
#[macro_export]
macro_rules! errln {
    ($($t:tt)*) => {{
        eprint!("{} ", $crate::Style::ErrorPrefix.paint("error:"));
        eprintln!($($t)*);
    }};
}

/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that a checkmark prefix is
/// printed before the message.
#[macro_export]
macro_rules! successln {
    ($($t:tt)*) => {{
        eprint!("{} ", $crate::Style::SuccessPrefix.paint("✓"));
        eprintln!($($t)*);
    }};
}
