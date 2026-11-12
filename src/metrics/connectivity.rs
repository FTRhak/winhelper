//! Internet connectivity from the system's own network status
//! (`GetNetworkConnectivityHint`, Windows 10 2004+).
//!
//! The function is looked up at run time so the exe still starts on older
//! builds; there the status is unknown and the UI falls back to the result of
//! the last IP lookup.

use windows::core::{s, w};
use windows::Win32::Foundation::{NO_ERROR, WIN32_ERROR};
use windows::Win32::Networking::WinSock::NL_NETWORK_CONNECTIVITY_HINT;
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

type HintFn = unsafe extern "system" fn(*mut NL_NETWORK_CONNECTIVITY_HINT) -> WIN32_ERROR;

/// Maps a connectivity level to online / offline; `None` = unknown.
fn classify(level: i32) -> Option<bool> {
    match level {
        // InternetAccess, ConstrainedInternetAccess (captive portal, metered limit).
        3 | 4 => Some(true),
        // None, LocalAccess.
        1 | 2 => Some(false),
        // Unknown, Hidden.
        _ => None,
    }
}

pub struct Connectivity {
    hint: Option<HintFn>,
}

impl Connectivity {
    pub fn new() -> Self {
        // SAFETY: iphlpapi stays loaded for the process lifetime (it is also a
        // static import); the export has had this signature since it was added.
        let hint = unsafe {
            LoadLibraryExW(w!("iphlpapi.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                .ok()
                .and_then(|module| GetProcAddress(module, s!("GetNetworkConnectivityHint")))
                .map(|f| std::mem::transmute::<_, HintFn>(f))
        };
        Self { hint }
    }

    /// `Some(true)` with internet access, `Some(false)` without, `None` unknown.
    pub fn online(&self) -> Option<bool> {
        let hint = self.hint?;
        let mut value = NL_NETWORK_CONNECTIVITY_HINT::default();
        // SAFETY: `value` is a valid out parameter of the expected type.
        let status = unsafe { hint(&mut value) };
        if status != NO_ERROR {
            return None;
        }
        classify(value.ConnectivityLevel.0)
    }
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn levels_map_to_online_state() {
        assert_eq!(classify(0), None);
        assert_eq!(classify(1), Some(false));
        assert_eq!(classify(2), Some(false));
        assert_eq!(classify(3), Some(true));
        assert_eq!(classify(4), Some(true));
        assert_eq!(classify(5), None);
    }
}
