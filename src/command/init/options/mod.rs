#[cfg(feature = "composition-js")]
mod project_authentication;
#[cfg(feature = "composition-js")]
mod project_graphid;
#[cfg(feature = "composition-js")]
mod project_name;
#[cfg(feature = "composition-js")]
mod project_organization;
#[cfg(feature = "composition-js")]
mod project_template;
#[cfg(feature = "composition-js")]
mod project_type;
#[cfg(feature = "composition-js")]
mod project_use_case;
#[cfg(feature = "composition-js")]
mod schema_name;

#[cfg(feature = "composition-js")]
pub(crate) use project_authentication::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_graphid::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_name::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_organization::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_template::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_type::*;
#[cfg(feature = "composition-js")]
pub(crate) use project_use_case::*;
#[cfg(feature = "composition-js")]
pub(crate) use schema_name::*;
