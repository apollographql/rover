#[cfg(not(target_env = "musl"))]
mod install;
mod manifest;
#[cfg(not(target_env = "musl"))]
mod spellings;
