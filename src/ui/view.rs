//! What the panel shows, decoupled from where it comes from.
//!
//! The metrics thread fills CPU / RAM / GPU (`window::on_metrics`); the network
//! thread fills `net` (`window::on_netinfo`). Until data arrives every field is
//! `None` and the panel renders its "unavailable" state.

pub use crate::metrics::Memory;
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
    }
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

    fn info(lan: Option<&str>, public: Option<(&str, Option<&str>, Option<&str>)>) -> NetInfo {
        NetInfo {
            lan_ip: lan.map(Into::into),
            public: public.map(|(ip, loc, code)| crate::net::Geo {
                ip: ip.into(),
                location: loc.map(Into::into),
                country_code: code.map(Into::into),
            }),
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
        };
        assert_eq!(net, fresh);

        // Offline: public values kept but stale; LAN follows the adapter.
        net.update(info(None, None));
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
