//! `serde_stringify` allows you to serialize/deserialize
//! a struct with the `Display`/`FromStr` implementations
//! if it does not implement `Serialize`/`Deserialize`
//! code taken from this: https://github.com/serde-rs/serde/issues/1316
//! and can be used by annotating a field with
//! #[serde(serialize_with = "from_display")]
use std::fmt::Display;

use serde::Serializer;

pub fn from_display<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    if let Some(value) = value {
        serializer.collect_str(value)
    } else {
        serializer.serialize_none()
    }
}
