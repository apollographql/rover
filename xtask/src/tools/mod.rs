mod cargo;
mod git;
mod make;
mod npm;
mod runner;

#[cfg(target_os = "macos")]
mod xcrun;

pub(crate) use cargo::CargoRunner;
pub(crate) use git::GitRunner;
pub(crate) use make::MakeRunner;
pub(crate) use npm::NpmRunner;
pub(crate) use runner::Runner;

#[cfg(target_os = "macos")]
pub(crate) use xcrun::XcrunRunner;

#[cfg(not(windows))]
mod lychee;

#[cfg(not(windows))]
pub(crate) use lychee::LycheeRunner;
