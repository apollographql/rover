#[cfg(not(target_env = "gnu"))]
mod no_compose;

#[cfg(not(target_env = "gnu"))]
pub(crate) use no_compose::Compose;

#[cfg(target_os = "linux")]
#[cfg(target_env = "gnu")]
mod do_compose;

#[cfg(target_os = "linux")]
#[cfg(target_env = "gnu")]
pub(crate) use do_compose::Compose;
