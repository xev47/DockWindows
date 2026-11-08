#[cfg(windows)]
pub struct Thumbnail {
    handle: isize,
}

#[cfg(windows)]
impl Thumbnail {
    pub fn register(destination: isize, source: isize) -> Result<Self, String> {
        use windows_sys::Win32::Graphics::Dwm::DwmRegisterThumbnail;

        if destination == 0 || source == 0 {
            return Err("Invalid destination or source window handle.".to_owned());
        }

        let mut handle = 0;
        let result = unsafe { DwmRegisterThumbnail(destination as _, source as _, &mut handle) };
        if result < 0 {
            return Err(format!(
                "DwmRegisterThumbnail failed with HRESULT 0x{:08X}.",
                result as u32
            ));
        }
        Ok(Self { handle })
    }

    pub fn update_destination(&mut self, rect: [i32; 4]) -> Result<(), String> {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::Graphics::Dwm::{
            DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
            DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE,
        };

        let properties = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE,
            rcDestination: RECT {
                left: rect[0],
                top: rect[1],
                right: rect[2],
                bottom: rect[3],
            },
            rcSource: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            opacity: 255,
            fVisible: 1,
            fSourceClientAreaOnly: 0,
        };
        let result = unsafe { DwmUpdateThumbnailProperties(self.handle, &properties) };
        if result < 0 {
            return Err(format!(
                "DwmUpdateThumbnailProperties failed with HRESULT 0x{:08X}.",
                result as u32
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for Thumbnail {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Dwm::DwmUnregisterThumbnail;

        unsafe {
            DwmUnregisterThumbnail(self.handle);
        }
    }
}

#[cfg(not(windows))]
pub struct Thumbnail;

#[cfg(not(windows))]
impl Thumbnail {
    pub fn register(_destination: isize, _source: isize) -> Result<Self, String> {
        Err("Window thumbnails are only supported on Windows.".to_owned())
    }

    pub fn update_destination(&mut self, _rect: [i32; 4]) -> Result<(), String> {
        Err("Window thumbnails are only supported on Windows.".to_owned())
    }
}
