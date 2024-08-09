#[macro_export]
macro_rules! infoln {
    ($fmt_str:literal) => {{
        let marker = rover_std::Style::HintPrefix.paint("==>");
        eprintln!("{marker} {}", $fmt_str);
    }};

    ($fmt_str:literal, $($args:expr),*) => {{
        let marker = rover_std::Style::HintPrefix.paint("==>");
        let fmt = format!($fmt_str, $($args),*);
        eprintln!("{marker} {fmt}");
    }};
}

#[macro_export]
macro_rules! warnln {
    ($fmt_str:literal) => {{
        let marker = rover_std::Style::WarningPrefix.paint("==>");
        eprintln!("{marker} {}", $fmt_str);
    }};

    ($fmt_str:literal, $($args:expr),*) => {{
        let marker = rover_std::Style::WarningPrefix.paint("==>");
        let fmt = format!($fmt_str, $($args),*);
        eprintln!("{marker} {fmt}");
    }};
}

#[macro_export]
macro_rules! errln {
    ($fmt_str:literal) => {{
        let marker = rover_std::Style::ErrorPrefix.paint("==>");
        eprintln!("{marker} {}", $fmt_str);
    }};

    ($fmt_str:literal, $($args:expr),*) => {{
        let marker = rover_std::Style::ErrorPrefix.paint("==>");
        let fmt = format!($fmt_str, $($args),*);
        eprintln!("{marker} {fmt}");
    }};
}
