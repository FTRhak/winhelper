//! Network identity lookup on a background thread (PLAN.md §4, §5.4–5.5).
//!
//! `Lookup` reads the LAN IP and asks the geo providers for the public IP and
//! its location, once at start and again on every `refresh` (the
//! "Refresh IP now" menu command). Each result is stored as the latest
//! `NetInfo` and announced with `WM_APP_NETINFO`.

mod geo;
mod http;
mod local;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::win::Event;
pub use geo::Geo;

/// Posted to the panel when a lookup finished; fetch it with `Lookup::take_latest`.
pub const WM_APP_NETINFO: u32 = WM_APP + 4;

/// Result of one lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetInfo {
    pub lan_ip: Option<String>,
    /// `None` when every provider failed (offline, blocked, rate limited).
    pub public: Option<Geo>,
}

struct Shared {
    latest: Mutex<Option<NetInfo>>,
    /// A refresh was requested and has not started yet.
    pending: AtomicBool,
    /// A lookup is running (it may block on the network for several seconds).
    busy: AtomicBool,
    stop: AtomicBool,
    wake: Event,
}

/// Handle to the lookup thread. Dropping it stops the thread.
pub struct Lookup {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Lookup {
    /// Starts the thread, which looks up once right away. `None` if it cannot
    /// be started; the network rows then stay empty.
    pub fn start(panel: HWND) -> Option<Self> {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            pending: AtomicBool::new(true),
            busy: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            wake: Event::new().ok()?,
        });
        // HWND is not Send; window handles are global, so pass the raw value.
        let panel = panel.0 as isize;
        let worker = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("network".into())
            .spawn(move || run(&worker, panel))
            .ok()?;
        Some(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Requests a new lookup. Requests made while one runs collapse into a
    /// single follow-up lookup. Safe to call inside `state::with`.
    pub fn refresh(&self) {
        self.shared.pending.store(true, Ordering::SeqCst);
        self.shared.wake.set();
    }

    pub fn take_latest(&self) -> Option<NetInfo> {
        self.shared.latest.lock().ok()?.take()
    }
}

impl Drop for Lookup {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake.set();
        let Some(thread) = self.thread.take() else {
            return;
        };
        // An idle thread exits at once. One stuck in a request could hold up
        // shutdown for the full timeouts, so leave it; process exit ends it.
        // SeqCst on `stop` and `busy` means that if we see it idle here, it
        // sees `stop` before starting another lookup.
        if !self.shared.busy.load(Ordering::SeqCst) {
            let _ = thread.join();
        }
    }
}

fn run(shared: &Shared, panel: isize) {
    let mut session = None;
    loop {
        if !shared.pending.swap(false, Ordering::SeqCst) {
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }
            // Woken by `refresh` or on stop; re-check both.
            shared.wake.wait(Duration::MAX);
            continue;
        }
        shared.busy.store(true, Ordering::SeqCst);
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        // Retried on every refresh until it works (e.g. WinHTTP unavailable).
        if session.is_none() {
            session = http::Session::open().ok();
        }
        let info = NetInfo {
            lan_ip: local::lan_ip(),
            public: session.as_ref().and_then(geo::lookup),
        };
        if let Ok(mut latest) = shared.latest.lock() {
            *latest = Some(info);
        }
        shared.busy.store(false, Ordering::SeqCst);
        // SAFETY: posting to a window of another thread is allowed; it fails
        // harmlessly once the window is gone.
        unsafe {
            let _ = PostMessageW(HWND(panel as *mut _), WM_APP_NETINFO, WPARAM(0), LPARAM(0));
        }
    }
}

/// Queries the real providers: `cargo test -- --ignored --nocapture`.
/// Sends this machine's IP to the providers in `geo.rs`.
#[cfg(test)]
mod live {
    #[test]
    #[ignore = "network access; reveals the public IP to the geo providers"]
    fn looks_up_real_network() {
        let lan = super::local::lan_ip();
        let session = super::http::Session::open().expect("WinHTTP session");
        let public = super::geo::lookup(&session);
        println!("lan {lan:?}  public {public:?}");
        assert!(public.is_some());
    }
}
