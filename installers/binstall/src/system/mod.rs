#[cfg(not(windows))]
pub(crate) mod unix;

#[cfg(windows)]
pub(crate) mod windows;
