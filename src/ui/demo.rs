//! Debug-only sample data reproducing the states in design/WinHelper.bl
//! ("WinHelper · Minimal"), so the visual design can be checked without real load.
//!
//! `WINHELPER_DEMO=normal|high|offline` fills the strip and tooltip;
//! `WINHELPER_THEME=dark|light` overrides the system theme. Neither exists in
//! release builds.

use super::theme::Theme;
use super::view::{Disk, Memory, NetView, PanelView, Traffic};

const GIB: u64 = 1 << 30;
const KIB: f64 = 1024.0;
const MIB: f64 = 1024.0 * 1024.0;

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
        updated: Some("14:02".into()),
    };
    Some(match kind.as_str() {
        "high" => PanelView {
            cpu: Some(91.0),
            ram: Some(mem(125)),
            gpu: None,
            cpu_temp: Some(92.0),
            gpu_temp: None,
            disk: Some(Disk {
                active: 74.0,
                read: 85.0 * MIB,
                write: 40.0 * MIB,
            }),
            traffic: Some(Traffic {
                down: 24.8 * MIB,
                up: 1.6 * MIB,
            }),
            online: Some(true),
            net,
        },
        "offline" => PanelView {
            cpu: Some(7.0),
            ram: Some(mem(66)),
            gpu: Some(3.0),
            cpu_temp: Some(41.0),
            gpu_temp: Some(38.0),
            disk: Some(Disk {
                active: 2.0,
                read: 0.1 * MIB,
                write: 0.0,
            }),
            traffic: Some(Traffic::default()),
            online: Some(false),
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
            cpu_temp: Some(58.0),
            gpu_temp: Some(51.0),
            disk: Some(Disk {
                active: 12.0,
                read: 4.1 * MIB,
                write: 0.3 * MIB,
            }),
            traffic: Some(Traffic {
                down: 1.2 * MIB,
                up: 84.0 * KIB,
            }),
            online: Some(true),
            net,
        },
    })
}
