#[cfg(not(target_env = "musl"))]
mod install;
#[cfg(not(target_env = "musl"))]
mod lockfile;
mod manifest;
#[cfg(not(target_env = "musl"))]
mod offline;
#[cfg(not(target_env = "musl"))]
mod spellings;
