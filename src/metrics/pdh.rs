//! Thin RAII wrapper over a PDH query with English counter paths
//! (`PdhAddEnglishCounterW`, so localized Windows works).

use windows::core::PCWSTR;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA,
    PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_MORE_DATA,
};

/// Owned PDH query handle; closed on drop.
pub struct Query(isize);

/// A counter added to a `Query`; valid while the query lives.
#[derive(Clone, Copy)]
pub struct Counter(isize);

impl Query {
    pub fn open() -> Option<Self> {
        let mut handle = 0;
        // SAFETY: valid out-pointer.
        (unsafe { PdhOpenQueryW(None, 0, &mut handle) } == 0).then_some(Self(handle))
    }

    /// Adds an English counter path; `None` if this machine does not have it.
    pub fn add(&self, path: PCWSTR) -> Option<Counter> {
        let mut counter = 0;
        // SAFETY: the query handle is valid and `path` is NUL-terminated.
        (unsafe { PdhAddEnglishCounterW(self.0, path, 0, &mut counter) } == 0)
            .then_some(Counter(counter))
    }

    /// Takes a sample of every counter. Rate counters need two collections
    /// before they report a value.
    pub fn collect(&self) -> bool {
        // SAFETY: the query handle is valid.
        unsafe { PdhCollectQueryData(self.0) == 0 }
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: we own the query and close it exactly once.
        unsafe {
            PdhCloseQuery(self.0);
        }
    }
}

/// The value of a single-instance counter. `None` if PDH has no valid value yet.
pub fn value(counter: Counter) -> Option<f64> {
    let mut value = PDH_FMT_COUNTERVALUE::default();
    // SAFETY: the counter handle is valid and `value` is a valid out-pointer.
    let status =
        unsafe { PdhGetFormattedCounterValue(counter.0, PDH_FMT_DOUBLE, None, &mut value) };
    if status != 0 || !matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA) {
        return None;
    }
    // SAFETY: PDH_FMT_DOUBLE fills the double member.
    Some(unsafe { value.Anonymous.doubleValue })
}

/// Every instance of a wildcard counter as `(instance name, value)`. `buffer`
/// is reused between calls. `None` if PDH fails.
pub fn array(counter: Counter, buffer: &mut Vec<u64>) -> Option<Vec<(String, f64)>> {
    // Instances come and go (processes, devices), so the size can change between
    // the size query and the fetch; retry a few times.
    for _ in 0..3 {
        let (mut size, mut count) = (0u32, 0u32);
        // SAFETY: size query with no buffer, as documented.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(counter.0, PDH_FMT_DOUBLE, &mut size, &mut count, None)
        };
        if status != PDH_MORE_DATA {
            return None;
        }
        // u64 elements keep the buffer 8-byte aligned for the items.
        buffer.clear();
        buffer.resize((size as usize).div_ceil(8), 0);
        let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        // SAFETY: the buffer holds at least `size` bytes, suitably aligned.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                counter.0,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(items),
            )
        };
        if status == PDH_MORE_DATA {
            continue;
        }
        if status != 0 {
            return None;
        }
        // SAFETY: PDH wrote `count` items into the buffer; their name strings
        // point into the same buffer, which outlives this borrow.
        let items = unsafe { std::slice::from_raw_parts(items, count as usize) };
        let samples = items
            .iter()
            .filter(|item| {
                matches!(
                    item.FmtValue.CStatus,
                    PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
                )
            })
            // SAFETY: valid items carry a NUL-terminated name and a double value.
            .filter_map(|item| unsafe {
                let name = item.szName.to_string().ok()?;
                Some((name, item.FmtValue.Anonymous.doubleValue))
            })
            .collect();
        return Some(samples);
    }
    None
}
