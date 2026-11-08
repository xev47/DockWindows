use std::path::PathBuf;

#[cfg(not(windows))]
use eframe::egui::ColorImage;

pub struct ActiveApplication {
    pub path: PathBuf,
    pub title: String,
    pub window: isize,
}

impl ActiveApplication {
    pub fn path_key(&self) -> String {
        self.path.to_string_lossy().to_lowercase()
    }
}

/// Fenetres internes du shell Windows (bureau, barre des taches) : ce ne sont
/// pas des applications et elles ne doivent pas apparaitre dans le dock.
fn is_shell_window_class(class: &str) -> bool {
    matches!(
        class,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "TaskListThumbnailWnd"
    )
}

#[cfg(test)]
mod tests {
    use super::is_shell_window_class;

    #[test]
    fn shell_desktop_and_taskbar_windows_are_ignored() {
        assert!(is_shell_window_class("Progman"));
        assert!(is_shell_window_class("WorkerW"));
        assert!(is_shell_window_class("Shell_TrayWnd"));
        assert!(!is_shell_window_class("CabinetWClass"));
        assert!(!is_shell_window_class("Chrome_WidgetWin_1"));
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::iter::once;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::slice;

    use eframe::egui::ColorImage;
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetDC, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, DrawIconEx, EnumWindows, GetClassNameW, GetWindow, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW,
        SetForegroundWindow, ShowWindow, DI_NORMAL, GW_OWNER, SW_RESTORE, WM_CLOSE,
    };

    use super::{is_shell_window_class, ActiveApplication};

    const ICON_SIZE: i32 = 48;
    const ICON_BYTES: usize = ICON_SIZE as usize * ICON_SIZE as usize * 4;

    struct Enumeration {
        apps: Vec<ActiveApplication>,
        current_process_id: u32,
    }

    struct CurrentWindow {
        process_id: u32,
        handle: Option<isize>,
    }

    pub fn list() -> Vec<ActiveApplication> {
        let mut state = Enumeration {
            apps: Vec::new(),
            current_process_id: std::process::id(),
        };
        unsafe {
            EnumWindows(
                Some(enumerate_window),
                &mut state as *mut Enumeration as LPARAM,
            );
        }
        state.apps
    }

    pub fn current_window_handle() -> Option<isize> {
        let mut state = CurrentWindow {
            process_id: std::process::id(),
            handle: None,
        };
        unsafe {
            EnumWindows(
                Some(find_current_window),
                &mut state as *mut CurrentWindow as LPARAM,
            );
        }
        state.handle
    }

    unsafe extern "system" fn find_current_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam as *mut CurrentWindow);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        if process_id == state.process_id {
            state.handle = Some(hwnd as isize);
            return 0;
        }
        1
    }

    unsafe extern "system" fn enumerate_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam as *mut Enumeration);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }

        let title_len = GetWindowTextLengthW(hwnd);
        if title_len <= 0 {
            return 1;
        }
        let mut title_buffer = vec![0u16; title_len as usize + 1];
        let title_len = GetWindowTextW(hwnd, title_buffer.as_mut_ptr(), title_buffer.len() as i32);
        if title_len <= 0 {
            return 1;
        }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        if process_id == 0 || process_id == state.current_process_id {
            return 1;
        }

        let Some(path) = process_path(process_id) else {
            return 1;
        };

        let mut class_buffer = [0u16; 256];
        let class_len = GetClassNameW(hwnd, class_buffer.as_mut_ptr(), class_buffer.len() as i32);
        if class_len > 0
            && is_shell_window_class(&String::from_utf16_lossy(
                &class_buffer[..class_len as usize],
            ))
        {
            return 1;
        }

        let title = String::from_utf16_lossy(&title_buffer[..title_len as usize]);
        state.apps.push(ActiveApplication {
            path,
            title,
            window: hwnd as isize,
        });
        1
    }

    unsafe fn process_path(process_id: u32) -> Option<PathBuf> {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return None;
        }

        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        let succeeded =
            QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0;
        CloseHandle(process);

        succeeded.then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])))
    }

    pub fn activate(window: isize) {
        let hwnd = window as HWND;
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
    }

    pub fn close_window(window: isize) -> Result<(), String> {
        let hwnd = window as HWND;
        if hwnd.is_null() || unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) } == 0 {
            return Err("Windows n'a pas accepté la demande de fermeture.".to_owned());
        }
        Ok(())
    }

    pub fn close_application(path: &Path) -> Result<usize, String> {
        let path = path.to_string_lossy();
        let mut closed = 0;
        let mut errors = Vec::new();
        for app in list()
            .into_iter()
            .filter(|app| app.path.to_string_lossy().eq_ignore_ascii_case(&path))
        {
            match close_window(app.window) {
                Ok(()) => closed += 1,
                Err(error) => errors.push(error),
            }
        }

        if errors.is_empty() {
            Ok(closed)
        } else {
            Err(errors.join("\n"))
        }
    }

    pub fn load_icon(path: &Path) -> Option<ColorImage> {
        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(once(0)).collect();
        let mut file_info: SHFILEINFOW = unsafe { zeroed() };
        let result = unsafe {
            SHGetFileInfoW(
                wide_path.as_ptr(),
                0,
                &mut file_info,
                size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };
        if result == 0 || file_info.hIcon.is_null() {
            return None;
        }

        let icon = file_info.hIcon;
        let mut pixels = std::ptr::null_mut::<c_void>();
        let mut bitmap_info: BITMAPINFO = unsafe { zeroed() };
        bitmap_info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ICON_SIZE,
            biHeight: -ICON_SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..unsafe { zeroed() }
        };

        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            unsafe { DestroyIcon(icon) };
            return None;
        }
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        let bitmap = unsafe {
            CreateDIBSection(
                screen_dc,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut pixels,
                std::ptr::null_mut(),
                0,
            )
        };

        let mut old_bitmap = std::ptr::null_mut();
        let rgba = if memory_dc.is_null() || bitmap.is_null() || pixels.is_null() {
            None
        } else {
            old_bitmap = unsafe { SelectObject(memory_dc, bitmap as HGDIOBJ) };
            if old_bitmap.is_null() {
                None
            } else {
                let black = unsafe { render_icon(memory_dc, pixels, icon, 0) };
                let white = unsafe { render_icon(memory_dc, pixels, icon, 255) };
                black
                    .zip(white)
                    .map(|(black, white)| compose_rgba(&black, &white))
            }
        };

        unsafe {
            if !old_bitmap.is_null() && !memory_dc.is_null() {
                SelectObject(memory_dc, old_bitmap);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap as HGDIOBJ);
            }
            if !memory_dc.is_null() {
                DeleteDC(memory_dc);
            }
            ReleaseDC(std::ptr::null_mut(), screen_dc);
            DestroyIcon(icon);
        }

        rgba.map(|rgba| {
            ColorImage::from_rgba_unmultiplied([ICON_SIZE as usize, ICON_SIZE as usize], &rgba)
        })
    }

    unsafe fn render_icon(
        memory_dc: windows_sys::Win32::Graphics::Gdi::HDC,
        pixels: *mut c_void,
        icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
        background: u8,
    ) -> Option<Vec<u8>> {
        let buffer = slice::from_raw_parts_mut(pixels.cast::<u8>(), ICON_BYTES);
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[background, background, background, 255]);
        }
        if DrawIconEx(
            memory_dc,
            0,
            0,
            icon,
            ICON_SIZE,
            ICON_SIZE,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        ) == 0
        {
            return None;
        }
        GdiFlush();
        Some(slice::from_raw_parts(pixels.cast::<u8>(), ICON_BYTES).to_vec())
    }

    fn compose_rgba(black: &[u8], white: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(ICON_BYTES);
        for (dark, light) in black.chunks_exact(4).zip(white.chunks_exact(4)) {
            let alpha = 255u8.saturating_sub(
                light[0]
                    .saturating_sub(dark[0])
                    .max(light[1].saturating_sub(dark[1]))
                    .max(light[2].saturating_sub(dark[2])),
            );
            let channel = |value: u8| {
                if alpha == 0 {
                    0
                } else {
                    ((u16::from(value) * 255 + u16::from(alpha) / 2) / u16::from(alpha)) as u8
                }
            };
            rgba.extend_from_slice(&[channel(dark[2]), channel(dark[1]), channel(dark[0]), alpha]);
        }
        rgba
    }
}

#[cfg(windows)]
pub use windows::{
    activate, close_application, close_window, current_window_handle, list, load_icon,
};

#[cfg(not(windows))]
pub fn list() -> Vec<ActiveApplication> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn current_window_handle() -> Option<isize> {
    None
}

#[cfg(not(windows))]
pub fn activate(_window: isize) {}

#[cfg(not(windows))]
pub fn close_window(_window: isize) -> Result<(), String> {
    Err("La fermeture native n'est prise en charge que sous Windows.".to_owned())
}

#[cfg(not(windows))]
pub fn close_application(_path: &std::path::Path) -> Result<usize, String> {
    Err("La fermeture native n'est prise en charge que sous Windows.".to_owned())
}

#[cfg(not(windows))]
pub fn load_icon(_path: &std::path::Path) -> Option<ColorImage> {
    None
}
