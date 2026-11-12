//! What the strip and its tooltip show, decoupled from where it comes from.
//!
//! The metrics thread fills CPU / RAM / GPU, throughput and connectivity
//! (`window::on_metrics`); the network thread fills `net`
//! (`window::on_netinfo`). Until data arrives every field is `None` and the
//! UI renders its "unavailable" state.

pub use crate::metrics::{Disk, Memory, Traffic};
use crate::net::NetInfo;

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
    /// Local time of the last successful public lookup, "14:02".
    pub updated: Option<String>,
}

impl NetView {
    /// Applies a finished lookup. The LAN IP is always current. When every
    /// public provider failed, the last public values stay, grayed out (`stale`).
    pub fn update(&mut self, info: NetInfo) {
        self.lan_ip = info.lan_ip;
        let Some(geo) = info.public else {
            self.stale = true;
            return;
        };
        // An IP-only answer for the same IP keeps the location already known.
        let keep_location = geo.location.is_none() && self.public_ip.as_ref() == Some(&geo.ip);
        if !keep_location {
            self.location = geo.location;
            self.country_code = geo.country_code;
        }
        self.public_ip = Some(geo.ip);
        self.stale = false;
        self.updated = info.at;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelView {
    pub cpu: Option<f32>,
    pub ram: Option<Memory>,
    /// `None` when GPU counters are unavailable.
    pub gpu: Option<f32>,
    /// °C; `None` hides the temperature (no readable sensor).
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
    /// Active time and read / write rates; `None` when unavailable.
    pub disk: Option<Disk>,
    /// Download / upload rate; `None` until the first sample.
    pub traffic: Option<Traffic>,
    /// Internet access as Windows reports it; `None` when unknown.
    pub online: Option<bool>,
    pub net: NetView,
}

/// Internet status shown by the globe and the tooltip header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Online,
    Offline,
    Unknown,
}

impl PanelView {
    /// Windows' connectivity when known, else the outcome of the last IP lookup.
    pub fn status(&self) -> Status {
        match self.online {
            Some(true) => Status::Online,
            Some(false) => Status::Offline,
            None if self.net.stale => Status::Offline,
            None if self.net.public_ip.is_some() => Status::Online,
            None => Status::Unknown,
        }
    }
}

/// Rounds a load percentage for display and color classification.
pub fn round_percent(value: f32) -> u32 {
    if value.is_nan() {
        0
    } else {
        value.clamp(0.0, 100.0).round() as u32
    }
}

/// Tooltip form: "42 %", or "N/A" when the metric is unavailable.
pub fn percent_text(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{} %", round_percent(v)),
        None => "N/A".to_owned(),
    }
}

/// Strip form: "42%", or "N/A".
pub fn compact_percent(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{}%", round_percent(v)),
        None => "N/A".to_owned(),
    }
}

/// Rounds a temperature for display and color classification.
pub fn round_celsius(value: f32) -> u32 {
    if value.is_nan() {
        0
    } else {
        value.clamp(0.0, 999.0).round() as u32
    }
}

/// Strip form: "58°".
pub fn compact_temp(celsius: f32) -> String {
    format!("{}°", round_celsius(celsius))
}

/// Tooltip form: "58 °C".
pub fn temp_text(celsius: f32) -> String {
    format!("{} °C", round_celsius(celsius))
}

/// "R 4.1 · W 0.3 MB/s": both rates in the unit of the larger one, so they
/// compare at a glance.
pub fn disk_rates_text(read: f64, write: f64) -> String {
    let clean = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
    let (read, write) = (clean(read), clean(write));
    let (_, unit) = rate_parts(read.max(write));
    let div = match unit {
        "B/s" => 1.0,
        "KB/s" => 1024.0,
        "MB/s" => 1024.0 * 1024.0,
        _ => 1024.0 * 1024.0 * 1024.0,
    };
    let num = |v: f64| {
        let v = v / div;
        if div > 1.0 && v < 9.95 {
            format!("{v:.1}")
        } else {
            format!("{v:.0}")
        }
    };
    format!("R {} · W {} {unit}", num(read), num(write))
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

/// A byte rate split into number and unit, at most 4 + 4 characters so the
/// strip slots never overflow: ("1.2", "MB/s"), ("84", "KB/s"), ("0", "B/s").
/// Binary units (1 KB = 1024 B), one decimal below 10.
pub fn rate_parts(bytes_per_sec: f64) -> (String, &'static str) {
    const UNITS: [&str; 4] = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut v = if bytes_per_sec.is_finite() {
        bytes_per_sec.max(0.0)
    } else {
        0.0
    };
    let mut unit = 0;
    // 999.5 would round to "1000", one character too many.
    while v >= 999.5 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    let number = if unit > 0 && v < 9.95 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    };
    (number, UNITS[unit])
}

/// "1.2 MB/s".
pub fn rate_text(bytes_per_sec: f64) -> String {
    let (number, unit) = rate_parts(bytes_per_sec);
    format!("{number} {unit}")
}

/// Tooltip footer line.
pub fn footer_text(net: &NetView) -> String {
    match (&net.updated, net.stale) {
        (Some(at), false) => format!("IP updated {at} · right-click for menu"),
        (Some(at), true) => format!("IP lookup failed · last update {at}"),
        (None, true) => "IP lookup failed".to_owned(),
        (None, false) if net.public_ip.is_none() => "Looking up IP…".to_owned(),
        (None, false) => "Right-click for menu".to_owned(),
    }
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
        assert_eq!(compact_percent(Some(7.2)), "7%");
        assert_eq!(compact_percent(Some(100.0)), "100%");
        assert_eq!(compact_percent(None), "N/A");
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
    fn rate_scales_units_and_fits_slots() {
        let parts = |v: f64| {
            let (n, u) = rate_parts(v);
            (n, u.to_owned())
        };
        assert_eq!(parts(0.0), ("0".into(), "B/s".into()));
        assert_eq!(parts(999.0), ("999".into(), "B/s".into()));
        assert_eq!(parts(999.6), ("1.0".into(), "KB/s".into()));
        assert_eq!(parts(84.0 * 1024.0), ("84".into(), "KB/s".into()));
        assert_eq!(parts(1.2 * 1024.0 * 1024.0), ("1.2".into(), "MB/s".into()));
        assert_eq!(parts(24.8 * 1024.0 * 1024.0), ("25".into(), "MB/s".into()));
        assert_eq!(parts(9.96 * 1024.0), ("10".into(), "KB/s".into()));
        assert_eq!(parts(-5.0), ("0".into(), "B/s".into()));
        assert_eq!(parts(f64::NAN), ("0".into(), "B/s".into()));
        assert_eq!(parts(1e15).1, "GB/s");
        for v in [0.0, 999.4, 1023.0 * 1024.0, 5e9, 1e12] {
            let (n, u) = rate_parts(v);
            assert!(n.len() <= 4 && u.len() <= 4, "{v}: {n} {u}");
        }
        assert_eq!(rate_text(84.0 * 1024.0), "84 KB/s");
    }

    #[test]
    fn temps_format_both_forms() {
        assert_eq!(compact_temp(57.6), "58°");
        assert_eq!(temp_text(51.2), "51 °C");
        assert_eq!(round_celsius(f32::NAN), 0);
        assert_eq!(round_celsius(-5.0), 0);
    }

    #[test]
    fn disk_rates_share_the_larger_unit() {
        const MIB: f64 = 1024.0 * 1024.0;
        assert_eq!(disk_rates_text(4.1 * MIB, 0.3 * MIB), "R 4.1 · W 0.3 MB/s");
        assert_eq!(disk_rates_text(0.0, 120.0 * MIB), "R 0.0 · W 120 MB/s");
        assert_eq!(disk_rates_text(512.0, 0.0), "R 512 · W 0 B/s");
        assert_eq!(disk_rates_text(0.0, 0.0), "R 0 · W 0 B/s");
        assert_eq!(disk_rates_text(f64::NAN, -1.0), "R 0 · W 0 B/s");
    }

    #[test]
    fn status_prefers_windows_then_lookup() {
        let mut v = PanelView::default();
        assert_eq!(v.status(), Status::Unknown);
        v.net.public_ip = Some("203.0.113.42".into());
        assert_eq!(v.status(), Status::Online);
        v.net.stale = true;
        assert_eq!(v.status(), Status::Offline);
        v.online = Some(true);
        assert_eq!(v.status(), Status::Online);
        v.online = Some(false);
        assert_eq!(v.status(), Status::Offline);
    }

    #[test]
    fn footer_follows_lookup_state() {
        let mut net = NetView::default();
        assert_eq!(footer_text(&net), "Looking up IP…");
        net.public_ip = Some("203.0.113.42".into());
        net.updated = Some("14:02".into());
        assert_eq!(footer_text(&net), "IP updated 14:02 · right-click for menu");
        net.stale = true;
        assert_eq!(footer_text(&net), "IP lookup failed · last update 14:02");
        net.updated = None;
        assert_eq!(footer_text(&net), "IP lookup failed");
    }

    fn info(lan: Option<&str>, public: Option<(&str, Option<&str>, Option<&str>)>) -> NetInfo {
        NetInfo {
            lan_ip: lan.map(Into::into),
            public: public.map(|(ip, loc, code)| crate::net::Geo {
                ip: ip.into(),
                location: loc.map(Into::into),
                country_code: code.map(Into::into),
            }),
            at: Some("14:02".into()),
        }
    }

    #[test]
    fn net_update_success_then_failure() {
        let mut net = NetView::default();
        net.update(info(
            Some("192.168.1.15"),
            Some(("203.0.113.42", Some("Kyiv, Ukraine"), Some("UA"))),
        ));
        let fresh = NetView {
            public_ip: Some("203.0.113.42".into()),
            lan_ip: Some("192.168.1.15".into()),
            location: Some("Kyiv, Ukraine".into()),
            country_code: Some("UA".into()),
            stale: false,
            updated: Some("14:02".into()),
        };
        assert_eq!(net, fresh);

        // Offline: public values and their time kept but stale; LAN follows the adapter.
        let mut failed = info(None, None);
        failed.at = Some("14:12".into());
        net.update(failed);
        assert_eq!(
            net,
            NetView {
                lan_ip: None,
                stale: true,
                ..fresh.clone()
            }
        );

        // Back online: stale cleared.
        net.update(info(
            Some("192.168.1.15"),
            Some(("203.0.113.42", Some("Kyiv, Ukraine"), Some("UA"))),
        ));
        assert_eq!(net, fresh);
    }

    #[test]
    fn net_update_ip_only_answer() {
        let mut net = NetView::default();
        net.update(info(
            None,
            Some(("203.0.113.42", Some("Kyiv, Ukraine"), Some("UA"))),
        ));
        // Same IP without location: keep the known location.
        net.update(info(None, Some(("203.0.113.42", None, None))));
        assert_eq!(net.location.as_deref(), Some("Kyiv, Ukraine"));
        assert_eq!(net.country_code.as_deref(), Some("UA"));
        // New IP without location: the old location no longer applies.
        net.update(info(None, Some(("198.51.100.7", None, None))));
        assert_eq!(net.public_ip.as_deref(), Some("198.51.100.7"));
        assert_eq!(net.location, None);
        assert_eq!(net.country_code, None);
    }
}
