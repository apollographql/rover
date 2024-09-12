use interprocess::local_socket::{GenericFilePath, Name, ToFsName};

pub use follower::*;
pub use leader::*;
pub use types::*;

mod follower;
mod leader;
mod socket;
mod types;

pub(crate) fn create_socket_name(raw_socket_name: &str) -> std::io::Result<Name> {
    raw_socket_name.to_fs_name::<GenericFilePath>()
}
