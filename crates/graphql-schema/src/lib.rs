#![forbid(unsafe_code)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![warn(missing_docs, unreachable_pub, future_incompatible, rust_2018_idioms)]
#![doc(test(attr(deny(warnings))))]
#![doc(test(attr(allow(unused_extern_crates, unused_variables))))]

mod schema;
use schema::GraphQLClientSchema;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
