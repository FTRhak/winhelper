//! System load sampling on a background thread (PLAN.md §4, §5.1–5.3, §5.6).
//!
//! The sampler reads CPU, RAM, GPU and disk load, CPU / GPU temperature,
//! network throughput and internet connectivity, stores the latest
//! `MetricsSnapshot` in shared state and posts `WM_APP_METRICS` to the panel.
//! It samples every `FAST_INTERVAL` while the panel is active (focused or
//! hovered) and every `SLOW_INTERVAL` otherwise.

mod connectivity;
mod cpu;
mod disk;
mod gpu;
mod memory;
mod pdh;
mod thermal;
mod traffic;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::win::Event;
pub use disk::Disk;
pub use memory::Memory;
pub use traffic::Traffic;

/// Posted to the panel when a new snapshot is ready; fetch it with `Sampler::take_latest`.
pub const WM_APP_METRICS: u32 = WM_APP + 3;

/// Sampling period while the panel is focused or hovered.
pub const FAST_INTERVAL: Duration = Duration::from_secs(1);
/// Sampling period otherwise.
pub const SLOW_INTERVAL: Duration = Duration::from_secs(10);

/// One reading of every metric; `None` means unavailable.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MetricsSnapshot {
    pub cpu: Option<f32>,
    pub ram: Option<Memory>,
    pub gpu: Option<f32>,
    /// °C; `None` when no sensor is readable.
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
    /// Disk activity since the previous sample.
    pub disk: Option<Disk>,
    /// Throughput averaged since the previous sample.
    pub traffic: Option<Traffic>,
    /// Internet access per Windows; `None` when unknown.
    pub online: Option<bool>,
}

struct Shared {
    latest: Mutex<Option<MetricsSnapshot>>,
    active: AtomicBool,
    stop: AtomicBool,
    /// Set on stop and on becoming active, to cut the current wait short.
    wake: Event,
}

/// Handle to the sampler thread. Dropping it stops and joins the thread.
pub struct Sampler {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Sampler {
    /// Starts sampling for `panel`. `None` if the thread cannot be started, in
    /// which case the panel simply keeps showing N/A.
    pub fn start(panel: HWND) -> Option<Self> {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            active: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            wake: Event::new().ok()?,
        });
        // HWND is not Send; window handles are global, so pass the raw value.
        let panel = panel.0 as isize;
        let worker = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("metrics".into())
            .spawn(move || run(&worker, panel))
            .ok()?;
        Some(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Switches between the fast and slow interval. Becoming active samples
    /// right away if the last sample is at least `FAST_INTERVAL` old.
    pub fn set_active(&self, active: bool) {
        let was = self.shared.active.swap(active, Ordering::Relaxed);
        if active && !was {
            self.shared.wake.set();
        }
    }

    pub fn take_latest(&self) -> Option<MetricsSnapshot> {
        self.shared.latest.lock().ok()?.take()
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.set();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Time left until the next sample is due, or `None` if it is due now.
fn remaining(last: Instant, now: Instant, interval: Duration) -> Option<Duration> {
    (last + interval)
        .checked_duration_since(now)
        .filter(|d| !d.is_zero())
}

fn run(shared: &Shared, panel: isize) {
    let mut cpu = cpu::CpuSampler::new();
    let mut gpu = gpu::GpuSampler::open();
    let gpu_temp = gpu::GpuTemp::open();
    let mut cpu_temp = thermal::CpuTemp::open();
    let mut disk = disk::DiskSampler::open();
    let mut traffic = traffic::TrafficSampler::new();
    let connectivity = connectivity::Connectivity::new();
    let mut last = Instant::now();
    // The first sample comes after FAST_INTERVAL even when inactive, so the panel
    // does not show N/A for a whole slow period after startup.
    let mut first = true;

    while !shared.stop.load(Ordering::Relaxed) {
        let interval = if first || shared.active.load(Ordering::Relaxed) {
            FAST_INTERVAL
        } else {
            SLOW_INTERVAL
        };
        let now = Instant::now();
        if let Some(wait) = remaining(last, now, interval) {
            // Woken early on stop or activation; either way re-evaluate.
            shared.wake.wait(wait);
            continue;
        }
        first = false;
        last = now;

        let snapshot = MetricsSnapshot {
            cpu: cpu.sample(),
            ram: memory::read(),
            gpu: gpu.as_mut().and_then(gpu::GpuSampler::sample),
            cpu_temp: cpu_temp.as_mut().and_then(thermal::CpuTemp::sample),
            gpu_temp: gpu_temp.as_ref().and_then(gpu::GpuTemp::sample),
            disk: disk.as_mut().and_then(disk::DiskSampler::sample),
            traffic: traffic.sample(),
            online: connectivity.online(),
        };
        if let Ok(mut latest) = shared.latest.lock() {
            *latest = Some(snapshot);
        }
        // SAFETY: posting to a window of another thread is allowed; it fails
        // harmlessly once the window is gone.
        unsafe {
            let _ = PostMessageW(HWND(panel as *mut _), WM_APP_METRICS, WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_until_due() {
        let t0 = Instant::now();
        let s = Duration::from_secs;
        assert_eq!(remaining(t0, t0, s(1)), Some(s(1)));
        assert_eq!(remaining(t0, t0 + s(4), s(10)), Some(s(6)));
        assert_eq!(remaining(t0, t0 + s(1), s(1)), None);
        // Switching from slow to fast after 4 s: due immediately.
        assert_eq!(remaining(t0, t0 + s(4), FAST_INTERVAL), None);
    }
}

/// Reads the real counters on this machine: `cargo test -- --ignored --nocapture`.
#[cfg(test)]
mod live {
    #[test]
    #[ignore = "reads live system counters"]
    fn samples_real_counters() {
        let mut cpu = super::cpu::CpuSampler::new();
        let mut gpu = super::gpu::GpuSampler::open();
        let mut traffic = super::traffic::TrafficSampler::new();
        let mut disk = super::disk::DiskSampler::open();
        std::thread::sleep(super::FAST_INTERVAL);
        let cpu = cpu.sample();
        let ram = super::memory::read();
        let gpu = gpu.as_mut().and_then(super::gpu::GpuSampler::sample);
        let traffic = traffic.sample();
        let disk = disk.as_mut().and_then(super::disk::DiskSampler::sample);
        let cpu_temp = super::thermal::CpuTemp::open().and_then(|mut t| t.sample());
        let gpu_temp = super::gpu::GpuTemp::open().and_then(|t| t.sample());
        let online = super::connectivity::Connectivity::new().online();
        println!("cpu {cpu:?}  ram {ram:?}  gpu {gpu:?}  traffic {traffic:?}  online {online:?}");
        println!("disk {disk:?}  cpu temp {cpu_temp:?}  gpu temp {gpu_temp:?}");
        assert!(cpu.is_some_and(|v| (0.0..=100.0).contains(&v)));
        assert!(ram.is_some_and(|m| m.used <= m.total && m.total > 0));
        assert!(traffic.is_some());
    }
}
