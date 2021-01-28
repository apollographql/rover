#![forbid(unsafe_code)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![warn(missing_docs, unreachable_pub, future_incompatible, rust_2018_idioms)]
#![doc(test(attr(deny(warnings))))]
#![doc(test(attr(allow(unused_extern_crates, unused_variables))))]

mod constants;
mod normalization;
mod query;
pub mod schema;
mod type_qualifiers;
