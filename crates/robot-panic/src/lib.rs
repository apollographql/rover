//! Panic messages for humans by robots
//!
//! Handles panics by calling
//! [`std::panic::set_hook`](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
//! to make errors nice for humans.
//!
//! ## Why?
//! When you're building a CLI, polish is super important. Even though Rust is
//! pretty great at safety, it's not unheard of to access the wrong index in a
//! vector or have an assert fail somewhere.
//!
//! When an error eventually occurs, you probably will want to know about it. So
//! instead of just providing an error message on the command line, we can create a
//! call to action for people to submit a report.
//!
//! This should empower people to engage in communication, lowering the chances
//! people might get frustrated. And making it easier to figure out what might be
//! causing bugs.
//!
//! ### Default Output
//!
//! ```txt
//! thread 'main' panicked at 'oh no', src/bin/rover.rs:11:5
//! note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
//! ```
//!
//! ### Robot-Panic Output
//!
//! Houston, we have a problem. Rover crashed!
//! To help us diagnose the problem you can send us a crash report.
//!
//! You can submit an issue with the crash report at this link: https://github.com/apollographql/rover/issues/new?title=bug&assignees=&labels=bug+%F0%9F%90%9E%2C+triage&template=bug_report.md%3A+crashed+while+%3Cinsert+description+here%3E&body=%3C%21--%0A++Please+add+some+additional+information+about+what+you+were+trying+to+do+before+submitting+this+report%0A+--%3E+%0A%0A**Crash+Report**%0A+%60%60%60toml%0Aname+%3D+%27rover%27%0Aoperating_system+%3D+%27unix%3AUbuntu%27%0Acrate_version+%3D+%270.0.0%27%0Aexplanation+%3D+%27%27%27%0APanic+occurred+in+file+%27src%2Fbin%2Frover.rs%27+at+line+11%0A%27%27%27%0Acause+%3D+%27oh+no%27%0Amethod+%3D+%27Panic%27%0Abacktrace+%3D+%27%27%27%0A%0A+++0%3A+0x5575363b5003+-+std%3A%3Asys_common%3A%3Abacktrace%3A%3A__rust_begin_short_backtrace%3A%3Ahcc063f8de39c7379%0A+++1%3A+0x5575363b505d+-+std%3A%3Art%3A%3Alang_start%3A%3A%7B%7Bclosure%7D%7D%3A%3Ahf77d52b639388bce%0A+++2%3A+0x5575364a2e41+-+core%3A%3Aops%3A%3Afunction%3A%3Aimpls%3A%3A%3Cimpl+core%3A%3Aops%3A%3Afunction%3A%3AFnOnce%3CA%3E+for+%26F%3E%3A%3Acall_once%3A%3Ah6a3209f124be2235%0A++++++++++++++++at+%2Frustc%2F18bf6b4f01a6feaf7259ba7cdae58031af1b7b39%2Flibrary%2Fcore%2Fsrc%2Fops%2Ffunction.rs%3A259%0A+++++++++++++++++-+std%3A%3Apanicking%3A%3Atry%3A%3Ado_call%3A%3Ah88ce358792b64df0%0A++++++++++++++++at+%2Frustc%2F18bf6b4f01a6feaf7259ba7cdae58031af1b7b39%2Flibrary%2Fstd%2Fsrc%2Fpanicking.rs%3A373%0A+++++++++++++++++-+std%3A%3Apanicking%3A%3Atry%3A%3Ah6311c259678e50fc%0A++++++++++++++++at+%2Frustc%2F18bf6b4f01a6feaf7259ba7cdae58031af1b7b39%2Flibrary%2Fstd%2Fsrc%2Fpanicking.rs%3A337%0A+++++++++++++++++-+std%3A%3Apanic%3A%3Acatch_unwind%3A%3Ah56c5716807d659a1%0A++++++++++++++++at+%2Frustc%2F18bf6b4f01a6feaf7259ba7cdae58031af1b7b39%2Flibrary%2Fstd%2Fsrc%2Fpanic.rs%3A379%0A+++++++++++++++++-+std%3A%3Art%3A%3Alang_start_internal%3A%3Ah73711f37ecfcb277%0A++++++++++++++++at+%2Frustc%2F18bf6b4f01a6feaf7259ba7cdae58031af1b7b39%2Flibrary%2Fstd%2Fsrc%2Frt.rs%3A51%0A+++3%3A+0x5575363b4fc2+-+main%0A+++4%3A+0x7fa1956800b3+-+__libc_start_main%0A+++5%3A+0x5575363b40be+-+_start%0A+++6%3A++++++++0x0+-+%3Cunresolved%3E%27%27%27%0A%0A%60%60%60%0A
//!
//! We take privacy seriously, and do not perform any automated error collection. In order to improve the software, we rely on people to submit reports.
//!
//! Thanks for your patience!

pub mod report;
use report::{Method, Report};

use std::borrow::Cow;
use std::io::{Result as IoResult, Write};
use std::panic::PanicHookInfo;

use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

/// A convenient metadata struct that describes a crate
pub struct Metadata {
    /// The crate version
    pub version: Cow<'static, str>,

    /// The crate name
    pub name: Cow<'static, str>,

    /// The list of authors of the crate
    pub authors: Cow<'static, str>,

    /// The URL of the crate's website
    pub homepage: Cow<'static, str>,

    /// The URL of the crate's repo
    pub repository: Cow<'static, str>,
}

/// `robot-panic` initialisation macro
///
/// You can either call this macro with no arguments `setup_panic!()` or
/// with a Metadata struct, if you don't want the error message to display
/// the values used in your `Cargo.toml` file.
///
/// The Metadata struct can't implement `Default` because of orphan rules, which
/// means you need to provide all fields for initialisation.
///
/// ```
/// use robot_panic::setup_panic;
///
/// setup_panic!(Metadata {
///     name: env!("CARGO_PKG_NAME").into(),
///     version: env!("CARGO_PKG_VERSION").into(),
///     authors: "My Company Support <support@mycompany.com>".into(),
///     homepage: "support.mycompany.com".into(),
///     repository: env!("CARGO_PKG_REPOSITORY").into()
/// });
/// ```
#[macro_export]
macro_rules! setup_panic {
    ($meta:expr) => {
        #[allow(unused_imports)]
        use std::panic::{self, PanicHookInfo};
        #[allow(unused_imports)]
        use $crate::{get_report, print_msg, Metadata};

        #[cfg(not(debug_assertions))]
        match ::std::env::var("RUST_BACKTRACE") {
            Err(_) => {
                panic::set_hook(Box::new(move |info: &PanicInfo| {
                    let crash_report = get_report(&$meta, info);
                    print_msg(&crash_report, &$meta)
                        .expect("robot-panic: printing error message to console failed");
                }));
            }
            Ok(_) => {}
        }
    };

    () => {
        #[allow(unused_imports)]
        use std::panic::{self, PanicInfo};
        #[allow(unused_imports)]
        use $crate::{get_report, print_msg, Metadata};

        #[cfg(not(debug_assertions))]
        match ::std::env::var("RUST_BACKTRACE") {
            Err(_) => {
                let meta = Metadata {
                    version: env!("CARGO_PKG_VERSION").into(),
                    name: env!("CARGO_PKG_NAME").into(),
                    authors: env!("CARGO_PKG_AUTHORS").replace(":", ", ").into(),
                    homepage: env!("CARGO_PKG_HOMEPAGE").into(),
                    repository: env!("CARGO_PKG_REPOSITORY").into(),
                };

                panic::set_hook(Box::new(move |info: &PanicInfo| {
                    let crash_report = get_report(&meta, info);
                    print_msg(&crash_report, &meta)
                        .expect("robot-panic: printing error message to console failed");
                }));
            }
            Ok(_) => {}
        }
    };
}

/// Utility function that prints a message to our human users
pub fn print_msg(crash_report: &Report, meta: &Metadata) -> IoResult<()> {
    let (_version, name, _authors, _homepage, repository) = (
        &meta.version,
        &meta.name,
        &meta.authors,
        &meta.homepage,
        &meta.repository,
    );

    // escape hatch for our use case ;)
    let name = if name == "rover" {
        "Rover".to_string()
    } else {
        name.to_string()
    };

    let stderr = BufferWriter::stderr(ColorChoice::Auto);
    let mut buffer = stderr.buffer();
    buffer.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;

    writeln!(&mut buffer, "Houston, we have a problem. {} crashed!", name)?;
    writeln!(
        &mut buffer,
        "To help us diagnose the \
         problem you can send us a crash report.\n",
    )?;
    let issue_link = if !repository.is_empty() {
        crash_report.get_github_issue(repository).ok()
    } else {
        None
    };

    if let Some(issue_link) = issue_link {
        writeln!(
            &mut buffer,
            "You can submit an \
                 issue with the crash report at this link: {}",
            &issue_link,
        )?;
    } else {
        let path = crash_report.persist();
        match path {
            Ok(path) => {
                writeln!(
                    &mut buffer,
                    "We have generated a report file at \"{}\". Submit an \
                         issue with the subject of \"{} Crash Report\" and include the \
                         report as an attachment.",
                    path, name
                )?;
            }
            Err(_) => {
                let crash_report = crash_report.serialize();
                match crash_report {
                    Some(crash_report) => {
                        writeln!(
                            &mut buffer,
                            "We have generated a report which you can submit to \
                        the authors of this tool.\n\n{}",
                            &crash_report
                        )?;
                    }
                    None => {
                        writeln!(
                            &mut buffer,
                            "Unfortunately we could not generate a crash report."
                        )?;
                    }
                }
            }
        }
    }

    writeln!(
        &mut buffer,
        "\nWe take privacy seriously, and do not perform any \
         automated error collection. In order to improve the software, we rely on \
         people to submit reports.\n"
    )?;

    writeln!(&mut buffer, "Thanks for your patience!")?;

    buffer.reset()?;

    stderr.print(&buffer).unwrap();
    Ok(())
}

/// Utility function which will handle dumping information to disk
pub fn get_report(meta: &Metadata, panic_info: &PanicHookInfo) -> Report {
    let message = match (
        panic_info.payload().downcast_ref::<&str>(),
        panic_info.payload().downcast_ref::<String>(),
    ) {
        (Some(s), _) => Some(s.to_string()),
        (_, Some(s)) => Some(s.to_string()),
        (None, None) => None,
    };

    let cause = match message {
        Some(m) => m,
        None => "Unknown".into(),
    };

    let expl = match panic_info.location() {
        Some(location) => {
            format!(
                "Panic occurred in file '{}' at line {}\n",
                location.file(),
                location.line()
            )
        }
        None => "Panic location unknown.\n".to_string(),
    };

    Report::new(&meta.name, &meta.version, Method::Panic, expl, cause)
}
