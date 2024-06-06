use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, Name, NameType, ToFsName, ToNsName,
};

pub use follower::*;
pub use leader::*;
pub(crate) use socket::*;
pub use types::*;

mod follower;
mod leader;
mod socket;
mod types;

pub(crate) fn create_socket_name(raw_socket_name: &str) -> std::io::Result<Name> {
    if GenericNamespaced::is_supported() {
        raw_socket_name.to_ns_name::<GenericNamespaced>()
    } else {
        raw_socket_name.to_fs_name::<GenericFilePath>()
    }
}
