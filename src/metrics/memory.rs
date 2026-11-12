//! Physical memory usage from `GlobalMemoryStatusEx` (PLAN.md §5.1).

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// Physical memory in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub used: u64,
    pub total: u64,
}

impl Memory {
    pub fn percent(self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used as f32 * 100.0 / self.total as f32
        }
    }
}

pub fn read() -> Option<Memory> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is a properly sized, initialized MEMORYSTATUSEX.
    unsafe { GlobalMemoryStatusEx(&mut status).ok()? };
    Some(Memory {
        used: status.ullTotalPhys.saturating_sub(status.ullAvailPhys),
        total: status.ullTotalPhys,
    })
}

#[cfg(test)]
mod tests {
    use super::Memory;

    #[test]
    fn zero_total_memory_is_zero_percent() {
        assert_eq!(Memory { used: 5, total: 0 }.percent(), 0.0);
    }

    #[test]
    fn percent_of_total() {
        assert_eq!(Memory { used: 1, total: 4 }.percent(), 25.0);
    }
}
