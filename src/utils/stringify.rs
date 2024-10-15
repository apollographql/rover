//! `serde_stringify` allows you to serialize/deserialize
//! a struct with the `Display`/`FromStr` implementations
//! if it does not implement `Serialize`/`Deserialize`
//! code taken from this: <https://github.com/serde-rs/serde/issues/1316>
//! and can be used by annotating a field with either
//! #[serde(serialize_with = "from_display")] or
//! #[serde(serialize_with = "option_from_display")]
//! depending on if the type you're serializing is nested in an Option
use std::fmt::Display;

use serde::Serializer;

pub fn option_from_display<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    if let Some(value) = value {
        from_display(value, serializer)
    } else {
        serializer.serialize_none()
    }
}

pub fn from_display<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    serializer.collect_str(value)
}
