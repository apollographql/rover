#[cfg(not(all(target_os = "linux", target_env = "musl")))]
mod plugin;
