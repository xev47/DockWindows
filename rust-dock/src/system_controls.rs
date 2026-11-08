pub fn open_wifi_networks() -> Result<(), String> {
    open::that("ms-availablenetworks:").map_err(|error| error.to_string())
}

#[cfg(windows)]
fn with_audio_endpoint<T>(
    action: impl FnOnce(
        &windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume,
    ) -> Result<T, String>,
) -> Result<T, String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    struct ComInitialization {
        uninitialize_on_drop: bool,
    }

    impl ComInitialization {
        fn new() -> Result<Self, String> {
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if result.is_err() && result != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
                return Err(windows::core::Error::from(result).to_string());
            }
            Ok(Self {
                uninitialize_on_drop: result.is_ok(),
            })
        }
    }

    impl Drop for ComInitialization {
        fn drop(&mut self) {
            if self.uninitialize_on_drop {
                unsafe { CoUninitialize() };
            }
        }
    }

    let _com = ComInitialization::new()?;
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|error| error.to_string())?
    };
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .map_err(|error| error.to_string())?
    };
    let endpoint: IAudioEndpointVolume = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .map_err(|error| error.to_string())?
    };
    action(&endpoint)
}

#[cfg(windows)]
pub fn master_volume() -> Result<f32, String> {
    with_audio_endpoint(|endpoint| unsafe {
        endpoint
            .GetMasterVolumeLevelScalar()
            .map_err(|error| error.to_string())
    })
}

#[cfg(not(windows))]
pub fn master_volume() -> Result<f32, String> {
    Err("Le réglage du volume système est uniquement disponible sous Windows.".to_owned())
}

#[cfg(windows)]
pub fn set_master_volume(volume: f32) -> Result<(), String> {
    with_audio_endpoint(|endpoint| unsafe {
        endpoint
            .SetMasterVolumeLevelScalar(volume.clamp(0.0, 1.0), std::ptr::null())
            .map_err(|error| error.to_string())
    })
}

#[cfg(not(windows))]
pub fn set_master_volume(_volume: f32) -> Result<(), String> {
    Err("Le réglage du volume système est uniquement disponible sous Windows.".to_owned())
}
