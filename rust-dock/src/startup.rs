#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::io;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "DockWindows";

    fn wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    fn status_error(status: u32) -> io::Error {
        io::Error::from_raw_os_error(status as i32)
    }

    pub fn is_enabled() -> io::Result<bool> {
        let run_key = wide_null(RUN_KEY);
        let value_name = wide_null(VALUE_NAME);
        let mut key: HKEY = null_mut();
        let open_status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                run_key.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            )
        };
        if open_status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if open_status != ERROR_SUCCESS {
            return Err(status_error(open_status));
        }

        let mut value_type = 0;
        let mut data_size = 0;
        let query_status = unsafe {
            RegQueryValueExW(
                key,
                value_name.as_ptr(),
                null_mut(),
                &mut value_type,
                null_mut(),
                &mut data_size,
            )
        };
        let close_status = unsafe { RegCloseKey(key) };

        if query_status != ERROR_SUCCESS && query_status != ERROR_FILE_NOT_FOUND {
            return Err(status_error(query_status));
        }
        if close_status != ERROR_SUCCESS {
            return Err(status_error(close_status));
        }
        Ok(query_status == ERROR_SUCCESS)
    }

    pub fn set_enabled(enabled: bool) -> io::Result<()> {
        let run_key = wide_null(RUN_KEY);
        let value_name = wide_null(VALUE_NAME);
        let mut key: HKEY = null_mut();

        if enabled {
            let executable = std::env::current_exe()?;
            let mut disposition = 0;
            let open_status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    run_key.as_ptr(),
                    0,
                    null_mut(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    null(),
                    &mut key,
                    &mut disposition,
                )
            };
            if open_status != ERROR_SUCCESS {
                return Err(status_error(open_status));
            }

            let mut command = vec![b'"' as u16];
            command.extend(executable.as_os_str().encode_wide());
            command.extend([b'"' as u16, 0]);
            let set_status = unsafe {
                RegSetValueExW(
                    key,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr().cast(),
                    (command.len() * size_of::<u16>()) as u32,
                )
            };
            let close_status = unsafe { RegCloseKey(key) };
            if set_status != ERROR_SUCCESS {
                return Err(status_error(set_status));
            }
            if close_status != ERROR_SUCCESS {
                return Err(status_error(close_status));
            }
            return Ok(());
        }

        let open_status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                run_key.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            )
        };
        if open_status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        if open_status != ERROR_SUCCESS {
            return Err(status_error(open_status));
        }

        let delete_status = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
        let close_status = unsafe { RegCloseKey(key) };
        if delete_status != ERROR_SUCCESS && delete_status != ERROR_FILE_NOT_FOUND {
            return Err(status_error(delete_status));
        }
        if close_status != ERROR_SUCCESS {
            return Err(status_error(close_status));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use windows::{is_enabled, set_enabled};

#[cfg(not(windows))]
pub fn is_enabled() -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Le démarrage automatique n'est pris en charge que sous Windows.",
    ))
}
