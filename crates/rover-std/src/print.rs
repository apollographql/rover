/// Prints to the standard output, with a newline.
///
/// Equivalent to the [`println!`] macro except that an info prefix is
/// printed before the message.
#[macro_export]
macro_rules! infoln {
    ($($t:tt)*) => {{
        print!("{} ", $crate::Style::InfoPrefix.paint("==>"));
        println!($($t)*);
    }};
}

/// Prints to the standard error, with a newline.
///
/// Equivalent to the [`eprintln!`] macro except that a warning prefix is
/// printed before the message.
#[macro_export]
macro_rules! warnln {
    ($($t:tt)*) => {{
        eprint!("{}: ", $crate::Style::WarningPrefix.paint("warning"));
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
        eprint!("{}: ", $crate::Style::ErrorPrefix.paint("error"));
        eprintln!($($t)*);
    }};
}
