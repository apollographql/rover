// most of this code is taken wholesale from this:
// https://github.com/rust-lang/rustup/blob/master/src/cli/self_update/windows.rs
use windows_registry::{CURRENT_USER, HSTRING};

use crate::{Installer, InstallerError};

/// Adds the downloaded binary in Installer to a Windows PATH
pub fn add_binary_to_path(installer: &Installer) -> Result<(), InstallerError> {
    let windows_path = get_windows_path_var()?;
    let bin_path = HSTRING::from(installer.get_bin_dir_path()?.as_std_path());
    if let Some(old_path) = windows_path {
        if let Some(new_path) = add_to_path(old_path, bin_path) {
            apply_new_path(new_path)?;
        }
    }

    Ok(())
}

// ---------------------------------------------
// https://en.wikipedia.org/wiki/Here_be_dragons
// ---------------------------------------------
// nothing is sacred beyond this point.

// Get the windows PATH variable out of the registry as a String. If
// this returns None then the PATH variable is not unicode and we
// should not mess with it.
fn get_windows_path_var() -> Result<Option<HSTRING>, InstallerError> {
    use windows_result::HRESULT;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_INVALID_DATA};

    let environment = CURRENT_USER.create("Environment")?;

    let reg_value = environment.get_hstring("PATH");
    match reg_value {
        Ok(val) => Ok(Some(val)),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_INVALID_DATA) => {
            eprintln!(
                "warn: the registry key HKEY_CURRENT_USER\\Environment\\PATH is not a string. \
                   Not modifying the PATH variable"
            );
            Ok(None)
        }
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND) => Ok(Some(HSTRING::new())),
        Err(e) => Err(e.into()),
    }
}

// Returns None if the existing old_path does not need changing, otherwise
// prepends the path_str to old_path, handling empty old_path appropriately.
fn add_to_path(old_path: HSTRING, path_str: HSTRING) -> Option<HSTRING> {
    if old_path.is_empty() {
        Some(path_str)
    } else if old_path
        .windows(path_str.len())
        .any(|path| *path == *path_str)
    {
        None
    } else {
        let mut new_path = path_str.to_os_string();
        new_path.push(";");
        new_path.push(old_path.to_os_string());
        Some(HSTRING::from(new_path))
    }
}

fn apply_new_path(new_path: HSTRING) -> Result<(), InstallerError> {
    use std::ptr;

    use windows_sys::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        },
    };

    let environment = CURRENT_USER.create("Environment")?;

    if new_path.is_empty() {
        environment.remove_value("PATH")?;
    } else {
        environment.set_expand_hstring("PATH", &new_path)?;
    }

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0 as WPARAM,
            c"Environment".as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_install_does_not_add_path_twice() {
        assert_eq!(
            None,
            super::add_to_path(
                HSTRING::from(r"c:\users\example\.mybinary\bin;foo"),
                HSTRING::from(r"c:\users\example\.mybinary\bin")
            )
        );
    }

    #[test]
    fn windows_install_does_add_path() {
        assert_eq!(
            Some(HSTRING::from(r"c:\users\example\.mybinary\bin;foo")),
            super::add_to_path(
                HSTRING::from("foo"),
                HSTRING::from(r"c:\users\example\.mybinary\bin")
            )
        );
    }

    #[test]
    fn windows_install_does_add_path_no_double_semicolon() {
        assert_eq!(
            Some(HSTRING::from(r"c:\users\example\.mybinary\bin;foo;bar;")),
            super::add_to_path(
                HSTRING::from("foo;bar;"),
                HSTRING::from(r"c:\users\example\.mybinary\bin")
            )
        );
    }
}
