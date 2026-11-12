//! Debug-only sample data reproducing the states in design/WinHelper.bl, so the
//! visual design can be checked before real data sources exist.
//!
//! `WINHELPER_DEMO=normal|high|offline` fills the panel; `WINHELPER_THEME=dark|light`
//! overrides the system theme. Neither exists in release builds.

use super::theme::Theme;
use super::view::{Memory, NetView, PanelView};

const GIB: u64 = 1 << 30;

pub fn theme_override() -> Option<Theme> {
    match std::env::var("WINHELPER_THEME").ok()?.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

pub fn view() -> Option<PanelView> {
    let kind = std::env::var("WINHELPER_DEMO").ok()?;
    let mem = |tenths: u64| Memory {
        used: GIB * tenths / 10,
        total: GIB * 16,
    };
    let net = NetView {
        public_ip: Some("203.0.113.42".into()),
        lan_ip: Some("192.168.1.15".into()),
        location: Some("Kyiv, Ukraine".into()),
        country_code: Some("UA".into()),
        stale: false,
    };
    Some(match kind.as_str() {
        "high" => PanelView {
            cpu: Some(91.0),
            ram: Some(mem(125)),
            gpu: None,
            net,
        },
        "offline" => PanelView {
            cpu: Some(7.0),
            ram: Some(mem(66)),
            gpu: Some(3.0),
            net: NetView {
                lan_ip: None,
                stale: true,
                ..net
            },
        },
        _ => PanelView {
            cpu: Some(42.0),
            ram: Some(mem(101)),
            gpu: Some(18.0),
            net,
        },
    })
}
