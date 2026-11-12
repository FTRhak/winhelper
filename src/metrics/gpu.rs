//! GPU load from the PDH `\GPU Engine(*)\Utilization Percentage` counter
//! (PLAN.md §5.3).
//!
//! Instances are per process and engine, e.g.
//! `pid_1234_luid_0x00000000_0x0000C2A1_phys_0_eng_3_engtype_3D`. Like Task
//! Manager, utilization is summed per engine across processes and the busiest
//! engine of any adapter is reported.
//!
//! The temperature comes from D3DKMT adapter performance data (`GpuTemp`).

use std::collections::HashMap;

use windows::core::w;
use windows::Wdk::Graphics::Direct3D::{
    D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo, D3DKMT_ADAPTERINFO,
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2, D3DKMT_QUERYADAPTERINFO,
    KMTQAITYPE_ADAPTERPERFDATA,
};

use super::pdh::{self, Counter, Query};

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

pub struct GpuSampler {
    query: Query,
    counter: Counter,
    /// Reused PDH item buffer.
    buffer: Vec<u64>,
}

impl GpuSampler {
    /// Opens the query and takes the baseline collection. `None` when the GPU
    /// counters are unavailable (old WDDM driver, some VMs / RDP sessions).
    pub fn open() -> Option<Self> {
        let query = Query::open()?;
        let counter = query.add(w!("\\GPU Engine(*)\\Utilization Percentage"))?;
        // Rate counters need two collections; this is the first.
        query.collect();
        Some(Self {
            query,
            counter,
            buffer: Vec::new(),
        })
    }

    /// Load since the previous call, in percent.
    pub fn sample(&mut self) -> Option<f32> {
        if !self.query.collect() {
            return None;
        }
        let samples = pdh::array(self.counter, &mut self.buffer)?;
        aggregate(samples.iter().map(|(n, v)| (n.as_str(), *v)))
    }
}

/// Temperature of the hottest GPU in °C from the driver's deci-Celsius readings,
/// ignoring adapters that report nothing (0, e.g. Microsoft Basic Render
/// Driver) or nonsense. `None` if no adapter has a plausible reading.
pub fn hottest(deci_celsius: impl IntoIterator<Item = u32>) -> Option<f32> {
    deci_celsius
        .into_iter()
        .map(|d| d as f32 / 10.0)
        .filter(|c| (1.0..=150.0).contains(c))
        .reduce(f32::max)
}

/// GPU temperature through D3DKMT adapter performance data (WDDM 2.4+), the
/// source Task Manager uses. Adapter handles are opened once and closed on drop.
pub struct GpuTemp {
    adapters: Vec<u32>,
}

impl GpuTemp {
    /// `None` when no adapter reports a temperature (old driver, VM, iGPU without
    /// a sensor), so the UI hides the GPU temperature.
    pub fn open() -> Option<Self> {
        let mut enumerate = D3DKMT_ENUMADAPTERS2::default();
        // SAFETY: a null adapter array asks for the count only.
        if unsafe { D3DKMTEnumAdapters2(&mut enumerate) }.is_err() || enumerate.NumAdapters == 0 {
            return None;
        }
        let mut infos = vec![D3DKMT_ADAPTERINFO::default(); enumerate.NumAdapters as usize];
        enumerate.pAdapters = infos.as_mut_ptr();
        // SAFETY: the array holds NumAdapters entries.
        if unsafe { D3DKMTEnumAdapters2(&mut enumerate) }.is_err() {
            return None;
        }
        infos.truncate(enumerate.NumAdapters as usize);
        let temp = Self {
            adapters: infos.iter().map(|a| a.hAdapter).collect(),
        };
        // Keep the handles only if a sensor answers now; drop closes them otherwise.
        temp.sample().map(|_| temp)
    }

    pub fn sample(&self) -> Option<f32> {
        hottest(self.adapters.iter().filter_map(|&adapter| {
            let mut data = D3DKMT_ADAPTER_PERFDATA::default();
            let mut query = D3DKMT_QUERYADAPTERINFO {
                hAdapter: adapter,
                Type: KMTQAITYPE_ADAPTERPERFDATA,
                pPrivateDriverData: &mut data as *mut _ as *mut _,
                PrivateDriverDataSize: std::mem::size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
            };
            // SAFETY: `data` is the documented output struct and outlives the call.
            unsafe { D3DKMTQueryAdapterInfo(&mut query) }
                .is_ok()
                .then_some(data.Temperature)
        }))
    }
}

impl Drop for GpuTemp {
    fn drop(&mut self) {
        for &adapter in &self.adapters {
            // SAFETY: each handle came from D3DKMTEnumAdapters2 and is closed once.
            unsafe {
                let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER { hAdapter: adapter });
            }
        }
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

    #[test]
    fn hottest_skips_missing_sensors() {
        assert_eq!(hottest([0, 512, 0]), Some(51.2));
        assert_eq!(hottest([480, 655]), Some(65.5));
        assert_eq!(hottest([0, 0]), None);
        assert_eq!(hottest([65535]), None);
        assert_eq!(hottest(std::iter::empty()), None);
    }
}
