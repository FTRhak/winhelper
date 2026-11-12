//! What the panel shows, decoupled from where it comes from.
//!
//! The metrics (M1/M2) and network (M3) threads will fill this in; until then
//! every field is `None` and the panel renders its "unavailable" state.

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetView {
    pub public_ip: Option<String>,
    pub lan_ip: Option<String>,
    /// "City, Country".
    pub location: Option<String>,
    /// ISO 3166 alpha-2 code shown next to the location, e.g. "UA".
    pub country_code: Option<String>,
    /// Last refresh failed; values are from an earlier successful lookup.
    pub stale: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelView {
    pub cpu: Option<f32>,
    pub ram: Option<Memory>,
    /// `None` when GPU counters are unavailable.
    pub gpu: Option<f32>,
    pub net: NetView,
}

/// Rounds a load percentage for display and color classification.
pub fn round_percent(value: f32) -> u32 {
    if value.is_nan() {
        0
    } else {
        value.clamp(0.0, 100.0).round() as u32
    }
}

/// "42 %", or "N/A" when the metric is unavailable.
pub fn percent_text(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{} %", round_percent(v)),
        None => "N/A".to_owned(),
    }
}

/// "10.1 / 16.0 GB" using binary gigabytes, like Task Manager.
pub fn memory_text(mem: Memory) -> String {
    const GIB: f64 = (1u64 << 30) as f64;
    format!(
        "{:.1} / {:.1} GB",
        mem.used as f64 / GIB,
        mem.total as f64 / GIB
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn percent_rounds_and_clamps() {
        assert_eq!(percent_text(Some(41.6)), "42 %");
        assert_eq!(percent_text(Some(-3.0)), "0 %");
        assert_eq!(percent_text(Some(100.4)), "100 %");
        assert_eq!(percent_text(None), "N/A");
        assert_eq!(round_percent(f32::NAN), 0);
    }

    #[test]
    fn memory_formats_in_gib() {
        let mem = Memory {
            used: GIB * 101 / 10,
            total: GIB * 16,
        };
        assert_eq!(memory_text(mem), "10.1 / 16.0 GB");
        assert_eq!(round_percent(mem.percent()), 63);
    }

    #[test]
    fn zero_total_memory_is_zero_percent() {
        assert_eq!(Memory { used: 5, total: 0 }.percent(), 0.0);
    }
}
