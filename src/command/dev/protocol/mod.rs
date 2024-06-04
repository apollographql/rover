pub use follower::*;
pub use leader::*;
pub(crate) use socket::*;
pub use types::*;

mod follower;
mod leader;
mod socket;
mod types;

macro_rules! create_socket_name {
    ($raw_socket_name:expr) => {
        if GenericFilePath::is_supported() {
            $raw_socket_name
                .clone()
                .to_fs_name::<GenericFilePath>()
                .unwrap()
        } else {
            $raw_socket_name
                .clone()
                .to_ns_name::<GenericNamespaced>()
                .unwrap()
        }
    };
}

pub(crate) use create_socket_name;
