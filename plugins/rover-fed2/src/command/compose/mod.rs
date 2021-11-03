#[cfg(not(feature = "composition-js"))]
mod no_compose;

#[cfg(not(feature = "composition-js"))]
pub(crate) use no_compose::Compose;

#[cfg(feature = "composition-js")]
mod do_compose;

#[cfg(feature = "composition-js")]
pub(crate) use do_compose::Compose;
