#[cfg(not(target_env = "musl"))]
mod from_lockfile;
#[cfg(not(target_env = "musl"))]
mod install;
#[cfg(not(target_env = "musl"))]
mod levels;
#[cfg(not(target_env = "musl"))]
mod lockfile;
mod manifest;
#[cfg(not(target_env = "musl"))]
mod offline;
#[cfg(not(target_env = "musl"))]
mod precedence;
#[cfg(not(target_env = "musl"))]
mod spellings;
