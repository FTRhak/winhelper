//! GPU load from the PDH `\GPU Engine(*)\Utilization Percentage` counter
//! (PLAN.md §5.3).
//!
//! Instances are per process and engine, e.g.
//! `pid_1234_luid_0x00000000_0x0000C2A1_phys_0_eng_3_engtype_3D`. Like Task
//! Manager, utilization is summed per engine across processes and the busiest
//! engine of any adapter is reported.

use std::collections::HashMap;

use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE, PDH_MORE_DATA,
};

/// Engine key of an instance name: everything from `luid_` up to `_engtype_`
/// (adapter, physical node and engine index), or `None` if it is not a GPU engine.
pub fn engine_key(instance: &str) -> Option<&str> {
    let start = instance.find("luid_")?;
    let rest = &instance[start..];
    let end = rest.find("_engtype_").unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Sums per-process samples per engine and returns the busiest engine, clamped
/// to 0–100 %. `None` if no sample names a GPU engine.
pub fn aggregate<'a>(samples: impl IntoIterator<Item = (&'a str, f64)>) -> Option<f32> {
    let mut engines: HashMap<&str, f64> = HashMap::new();
    for (name, value) in samples {
        if let Some(key) = engine_key(name) {
            let value = if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            };
            *engines.entry(key).or_default() += value;
        }
    }
    engines
        .into_values()
        .reduce(f64::max)
        .map(|max| max.clamp(0.0, 100.0) as f32)
}

/// Owned PDH query handle.
struct Query(isize);

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: we own the query and close it exactly once.
        unsafe {
            PdhCloseQuery(self.0);
        }
    }
}

pub struct GpuSampler {
    query: Query,
    counter: isize,
    /// Reused item buffer; u64 elements keep it 8-byte aligned for the items.
    buffer: Vec<u64>,
}

impl GpuSampler {
    /// Opens the query and takes the baseline collection. `None` when the GPU
    /// counters are unavailable (old WDDM driver, some VMs / RDP sessions).
    pub fn open() -> Option<Self> {
        let mut handle = 0;
        // SAFETY: plain FFI calls with valid out-pointers.
        unsafe {
            if PdhOpenQueryW(None, 0, &mut handle) != 0 {
                return None;
            }
            let query = Query(handle);
            let mut counter = 0;
            let path = w!("\\GPU Engine(*)\\Utilization Percentage");
            if PdhAddEnglishCounterW(query.0, path, 0, &mut counter) != 0 {
                return None;
            }
            // Rate counters need two collections; this is the first.
            PdhCollectQueryData(query.0);
            Some(Self {
                query,
                counter,
                buffer: Vec::new(),
            })
        }
    }

    /// Load since the previous call, in percent.
    pub fn sample(&mut self) -> Option<f32> {
        // SAFETY: the query and counter handles are valid while `self` lives.
        if unsafe { PdhCollectQueryData(self.query.0) } != 0 {
            return None;
        }
        // Instances come and go with processes, so the size can change between the
        // size query and the fetch; retry a few times.
        for _ in 0..3 {
            let (mut size, mut count) = (0u32, 0u32);
            // SAFETY: size query with no buffer, as documented.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    None,
                )
            };
            if status != PDH_MORE_DATA {
                return None;
            }
            self.buffer.clear();
            self.buffer.resize((size as usize).div_ceil(8), 0);
            let items = self
                .buffer
                .as_mut_ptr()
                .cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            // SAFETY: the buffer holds at least `size` bytes, suitably aligned.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
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
            let samples: Vec<(String, f64)> = items
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
            return aggregate(samples.iter().map(|(n, v)| (n.as_str(), *v)));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "luid_0x00000000_0x0000C2A1";
    const B: &str = "luid_0x00000000_0x0000D3B2";

    fn inst(pid: u32, luid: &str, eng: u32, ty: &str) -> String {
        format!("pid_{pid}_{luid}_phys_0_eng_{eng}_engtype_{ty}")
    }

    #[test]
    fn parses_engine_key() {
        assert_eq!(
            engine_key(&inst(1234, A, 3, "3D")),
            Some("luid_0x00000000_0x0000C2A1_phys_0_eng_3")
        );
        assert_eq!(
            engine_key("pid_1_luid_0x0_0x1_phys_0_eng_0_engtype_"),
            Some("luid_0x0_0x1_phys_0_eng_0")
        );
        assert_eq!(engine_key("_Total"), None);
    }

    #[test]
    fn sums_processes_per_engine_and_takes_max() {
        let names = [
            inst(1, A, 0, "3D"),
            inst(2, A, 0, "3D"),
            inst(1, A, 1, "Copy"),
            inst(3, B, 0, "3D"),
        ];
        let values = [20.0, 15.0, 30.0, 10.0];
        let samples = names.iter().map(String::as_str).zip(values);
        assert_eq!(aggregate(samples), Some(35.0));
    }

    #[test]
    fn engines_of_the_same_type_are_not_merged() {
        let names = [inst(1, A, 2, "Copy"), inst(1, A, 5, "Copy")];
        let samples = names.iter().map(String::as_str).zip([40.0, 40.0]);
        assert_eq!(aggregate(samples), Some(40.0));
    }

    #[test]
    fn clamps_and_ignores_junk() {
        let names = [inst(1, A, 0, "3D"), inst(2, A, 0, "3D"), "_Total".into()];
        let samples = names.iter().map(String::as_str).zip([80.0, 70.0, 500.0]);
        assert_eq!(aggregate(samples), Some(100.0));

        let names = [inst(1, A, 0, "3D")];
        let samples = names.iter().map(String::as_str).zip([f64::NAN]);
        assert_eq!(aggregate(samples), Some(0.0));
    }

    #[test]
    fn no_engines_is_none() {
        assert_eq!(aggregate(std::iter::empty()), None);
    }
}
