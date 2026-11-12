//! Total CPU load from `GetSystemTimes` deltas (PLAN.md §5.2, option A).

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;

/// Cumulative system times in 100 ns units. `kernel` includes `idle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// Load in percent between two readings: `1 − Δidle / (Δkernel + Δuser)`.
/// `None` when no time has passed (or the counters went backwards).
pub fn usage(prev: CpuTimes, cur: CpuTimes) -> Option<f32> {
    let idle = cur.idle.checked_sub(prev.idle)?;
    let total = cur.kernel.checked_sub(prev.kernel)? + cur.user.checked_sub(prev.user)?;
    if total == 0 {
        return None;
    }
    let busy = total.saturating_sub(idle);
    Some((busy as f64 * 100.0 / total as f64).clamp(0.0, 100.0) as f32)
}

fn read() -> Option<CpuTimes> {
    let (mut idle, mut kernel, mut user) = Default::default();
    // SAFETY: all three pointers are valid for the duration of the call.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()? };
    let ticks = |t: FILETIME| (t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64;
    Some(CpuTimes {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    })
}

/// Keeps the previous reading so each sample is the average since the last one.
pub struct CpuSampler {
    prev: Option<CpuTimes>,
}

impl CpuSampler {
    /// Takes the baseline reading.
    pub fn new() -> Self {
        Self { prev: read() }
    }

    pub fn sample(&mut self) -> Option<f32> {
        let cur = read()?;
        let load = self.prev.and_then(|prev| usage(prev, cur));
        self.prev = Some(cur);
        load
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(idle: u64, kernel: u64, user: u64) -> CpuTimes {
        CpuTimes { idle, kernel, user }
    }

    #[test]
    fn computes_busy_share() {
        // 1000 ticks elapsed (kernel 600, of which 250 idle, + user 400): 750 busy.
        let prev = times(1_000, 5_000, 3_000);
        let cur = times(1_250, 5_600, 3_400);
        assert_eq!(usage(prev, cur), Some(75.0));
    }

    #[test]
    fn idle_and_full_load() {
        let prev = times(0, 0, 0);
        assert_eq!(usage(prev, times(100, 100, 0)), Some(0.0));
        assert_eq!(usage(prev, times(0, 50, 50)), Some(100.0));
    }

    #[test]
    fn no_elapsed_time_or_backwards_is_none() {
        let t = times(10, 20, 30);
        assert_eq!(usage(t, t), None);
        assert_eq!(usage(t, times(5, 20, 30)), None);
    }
}
