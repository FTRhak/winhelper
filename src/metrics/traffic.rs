//! Network throughput from interface byte counters (`GetIfTable2`).
//!
//! Each sample sums the receive / transmit deltas of the physical interfaces
//! that are up since the previous sample and divides by the elapsed time.

use std::time::Instant;

use windows::Win32::Foundation::NO_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;

/// Download / upload rate in bytes per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Traffic {
    pub down: f64,
    pub up: f64,
}

/// Byte counters of one interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Counters {
    luid: u64,
    rx: u64,
    tx: u64,
}

/// `InterfaceAndOperStatusFlags` bits (`MIB_IF_ROW2`).
const FLAG_HARDWARE: u8 = 1 << 0;
const FLAG_FILTER: u8 = 1 << 1;

/// Rate between two counter sets taken `secs` apart. Only interfaces present
/// in both count, and a counter that went backwards (adapter reset) adds 0.
fn rate(prev: &[Counters], cur: &[Counters], secs: f64) -> Option<Traffic> {
    if secs <= 0.0 || !secs.is_finite() {
        return None;
    }
    let (mut rx, mut tx) = (0u64, 0u64);
    for c in cur {
        if let Some(p) = prev.iter().find(|p| p.luid == c.luid) {
            rx = rx.saturating_add(c.rx.saturating_sub(p.rx));
            tx = tx.saturating_add(c.tx.saturating_sub(p.tx));
        }
    }
    Some(Traffic {
        down: rx as f64 / secs,
        up: tx as f64 / secs,
    })
}

/// Whether an interface carries real traffic: a physical adapter that is up.
/// Filter and virtual interfaces would count the same bytes twice.
fn counts(flags: u8, if_type: u32, up: bool) -> bool {
    up && flags & FLAG_HARDWARE != 0
        && flags & FLAG_FILTER == 0
        && if_type != IF_TYPE_SOFTWARE_LOOPBACK
}

/// Owned `MIB_IF_TABLE2`, freed with `FreeMibTable`.
struct Table(*mut MIB_IF_TABLE2);

impl Table {
    fn get() -> Option<Self> {
        let mut table = std::ptr::null_mut();
        // SAFETY: on success the table is allocated by the API and freed in Drop.
        let status = unsafe { GetIfTable2(&mut table) };
        (status == NO_ERROR && !table.is_null()).then_some(Self(table))
    }

    fn rows(&self) -> &[MIB_IF_ROW2] {
        // SAFETY: the table holds `NumEntries` rows starting at `Table`.
        unsafe {
            let t = &*self.0;
            std::slice::from_raw_parts(t.Table.as_ptr(), t.NumEntries as usize)
        }
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        // SAFETY: allocated by GetIfTable2.
        unsafe { FreeMibTable(self.0 as *const _) };
    }
}

fn read() -> Option<Vec<Counters>> {
    let table = Table::get()?;
    let counters = table
        .rows()
        .iter()
        .filter(|r| {
            counts(
                r.InterfaceAndOperStatusFlags._bitfield,
                r.Type,
                r.OperStatus == IfOperStatusUp,
            )
        })
        .map(|r| Counters {
            // SAFETY: every bit pattern of the union is a valid u64.
            luid: unsafe { r.InterfaceLuid.Value },
            rx: r.InOctets,
            tx: r.OutOctets,
        })
        .collect();
    Some(counters)
}

pub struct TrafficSampler {
    last: Option<(Instant, Vec<Counters>)>,
}

impl TrafficSampler {
    /// Takes the baseline, so the first `sample` already has a rate.
    pub fn new() -> Self {
        Self {
            last: read().map(|c| (Instant::now(), c)),
        }
    }

    /// Rate since the previous call; `None` if the counters cannot be read.
    pub fn sample(&mut self) -> Option<Traffic> {
        let now = Instant::now();
        let cur = read()?;
        let result = self
            .last
            .as_ref()
            .and_then(|(at, prev)| rate(prev, &cur, now.duration_since(*at).as_secs_f64()));
        self.last = Some((now, cur));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(luid: u64, rx: u64, tx: u64) -> Counters {
        Counters { luid, rx, tx }
    }

    #[test]
    fn rate_sums_matching_interfaces() {
        let prev = [c(1, 1000, 100), c(2, 0, 0)];
        let cur = [c(1, 3000, 300), c(2, 1000, 50)];
        assert_eq!(
            rate(&prev, &cur, 2.0),
            Some(Traffic {
                down: 1500.0,
                up: 125.0
            })
        );
    }

    #[test]
    fn rate_skips_new_and_reset_interfaces() {
        let prev = [c(1, 5000, 5000)];
        // Interface 1 was reset (counters went back), 2 just appeared.
        let cur = [c(1, 10, 10), c(2, 9000, 9000)];
        assert_eq!(rate(&prev, &cur, 1.0), Some(Traffic::default()));
    }

    #[test]
    fn rate_needs_elapsed_time() {
        assert_eq!(rate(&[], &[], 0.0), None);
        assert_eq!(rate(&[], &[], f64::NAN), None);
    }

    #[test]
    fn counts_only_physical_up_interfaces() {
        assert!(counts(FLAG_HARDWARE, 6, true));
        assert!(!counts(FLAG_HARDWARE, 6, false));
        assert!(!counts(0, 6, true));
        assert!(!counts(FLAG_HARDWARE | FLAG_FILTER, 6, true));
        assert!(!counts(FLAG_HARDWARE, IF_TYPE_SOFTWARE_LOOPBACK, true));
    }
}
