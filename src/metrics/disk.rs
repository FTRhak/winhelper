//! Disk activity from the PDH `PhysicalDisk(_Total)` counters: active time
//! (100 − `% Idle Time`, like Task Manager) and read / write throughput summed
//! over all physical disks.

use windows::core::w;

use super::pdh::{self, Counter, Query};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Disk {
    /// Share of time the disks were busy, 0–100 %.
    pub active: f32,
    /// Bytes per second.
    pub read: f64,
    pub write: f64,
}

/// Active time from `% Idle Time`, clamped to 0–100 % (the raw counter can
/// overshoot 100 % idle on some drivers).
pub fn active_from_idle(idle: f64) -> f32 {
    if idle.is_finite() {
        (100.0 - idle).clamp(0.0, 100.0) as f32
    } else {
        0.0
    }
}

/// A byte rate as reported by PDH; negative or non-finite values become 0.
fn rate(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub struct DiskSampler {
    query: Query,
    idle: Counter,
    read: Counter,
    write: Counter,
}

impl DiskSampler {
    /// Opens the query and takes the baseline collection. `None` when the
    /// PhysicalDisk counters are missing (disabled with `diskperf -N`).
    pub fn open() -> Option<Self> {
        let query = Query::open()?;
        let idle = query.add(w!("\\PhysicalDisk(_Total)\\% Idle Time"))?;
        let read = query.add(w!("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec"))?;
        let write = query.add(w!("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec"))?;
        query.collect();
        Some(Self {
            query,
            idle,
            read,
            write,
        })
    }

    /// Activity since the previous call.
    pub fn sample(&mut self) -> Option<Disk> {
        if !self.query.collect() {
            return None;
        }
        Some(Disk {
            active: active_from_idle(pdh::value(self.idle)?),
            read: rate(pdh::value(self.read)?),
            write: rate(pdh::value(self.write)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_is_complement_of_idle() {
        assert_eq!(active_from_idle(88.0), 12.0);
        assert_eq!(active_from_idle(100.0), 0.0);
        assert_eq!(active_from_idle(104.5), 0.0);
        assert_eq!(active_from_idle(-3.0), 100.0);
        assert_eq!(active_from_idle(f64::NAN), 0.0);
    }

    #[test]
    fn rate_drops_junk() {
        assert_eq!(rate(4096.0), 4096.0);
        assert_eq!(rate(-1.0), 0.0);
        assert_eq!(rate(f64::INFINITY), 0.0);
    }
}
