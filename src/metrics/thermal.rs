//! CPU temperature from the ACPI thermal zones (PDH `Thermal Zone Information`),
//! readable without admin rights.
//!
//! Windows has no documented per-core sensor API; the thermal zones are what
//! the firmware exposes. On many desktops that is a motherboard or package
//! reading rather than the hottest core, and some machines have no zone at all,
//! in which case the temperature stays hidden.
//!
//! Many boards also expose placeholder zones stuck at a constant value (often
//! 301 K = 27.85 °C). A zone is only trusted once its reading has changed
//! (`Zones`), so such a placeholder never shows up as the CPU temperature.

use std::collections::HashMap;

use windows::core::w;

use super::pdh::{self, Counter, Query};

const KELVIN: f64 = 273.15;

/// Converts a raw zone reading to °C. `High Precision Temperature` is in tenths
/// of a kelvin, `Temperature` in whole kelvin; `scale` is 10 or 1. Readings
/// outside 1–150 °C (0 K, garbage) are `None`.
pub fn celsius(raw: f64, scale: f64) -> Option<f64> {
    let c = raw / scale - KELVIN;
    (c.is_finite() && (1.0..=150.0).contains(&c)).then_some(c)
}

/// Remembers each zone's first reading and whether it has moved since.
#[derive(Default)]
pub struct Zones {
    seen: HashMap<String, (f64, bool)>,
}

impl Zones {
    /// Feeds one sample of every zone (name, °C) and returns the hottest zone
    /// that has changed at least once, or `None` if none has yet.
    pub fn update(&mut self, zones: impl IntoIterator<Item = (String, f64)>) -> Option<f32> {
        let mut hottest: Option<f64> = None;
        for (name, c) in zones {
            let (first, live) = self.seen.entry(name).or_insert((c, false));
            *live |= (c - *first).abs() > 0.05;
            if *live {
                hottest = Some(hottest.map_or(c, |h| h.max(c)));
            }
        }
        hottest.map(|c| c as f32)
    }
}

pub struct CpuTemp {
    query: Query,
    counter: Counter,
    /// 10 for `High Precision Temperature`, 1 for `Temperature`.
    scale: f64,
    buffer: Vec<u64>,
    zones: Zones,
}

impl CpuTemp {
    /// `None` when there is no thermal zone with a plausible reading.
    pub fn open() -> Option<Self> {
        let query = Query::open()?;
        let (counter, scale) = match query.add(w!(
            "\\Thermal Zone Information(*)\\High Precision Temperature"
        )) {
            Some(c) => (c, 10.0),
            None => (
                query.add(w!("\\Thermal Zone Information(*)\\Temperature"))?,
                1.0,
            ),
        };
        let mut temp = Self {
            query,
            counter,
            scale,
            buffer: Vec::new(),
            zones: Zones::default(),
        };
        // Primes `zones` with the first readings.
        temp.sample();
        (!temp.zones.seen.is_empty()).then_some(temp)
    }

    /// Hottest live zone in °C; `None` until some zone has changed.
    pub fn sample(&mut self) -> Option<f32> {
        if !self.query.collect() {
            return None;
        }
        let raw = pdh::array(self.counter, &mut self.buffer)?;
        let scale = self.scale;
        self.zones.update(
            raw.into_iter()
                .filter_map(|(name, k)| Some((name, celsius(k, scale)?))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_units_and_rejects_junk() {
        assert!((celsius(3311.5, 10.0).unwrap() - 58.0).abs() < 0.01);
        assert!((celsius(331.0, 1.0).unwrap() - 57.85).abs() < 0.01);
        assert_eq!(celsius(0.0, 10.0), None);
        assert_eq!(celsius(99999.0, 10.0), None);
        assert_eq!(celsius(f64::NAN, 1.0), None);
    }

    fn sample(zones: &[(&str, f64)]) -> Vec<(String, f64)> {
        zones.iter().map(|&(n, c)| (n.to_owned(), c)).collect()
    }

    #[test]
    fn constant_zones_are_ignored_until_they_move() {
        let mut z = Zones::default();
        assert_eq!(z.update(sample(&[("TZ00", 27.85), ("CPU", 50.0)])), None);
        assert_eq!(z.update(sample(&[("TZ00", 27.85), ("CPU", 50.0)])), None);
        // The CPU zone moves: it counts from now on, even if it returns to its start.
        assert_eq!(
            z.update(sample(&[("TZ00", 27.85), ("CPU", 53.0)])),
            Some(53.0)
        );
        assert_eq!(
            z.update(sample(&[("TZ00", 27.85), ("CPU", 50.0)])),
            Some(50.0)
        );
        // Tiny jitter (below 0.05 °C) does not make a placeholder live.
        assert_eq!(
            z.update(sample(&[("TZ00", 27.87), ("CPU", 20.0)])),
            Some(20.0)
        );
    }

    #[test]
    fn hottest_live_zone_wins() {
        let mut z = Zones::default();
        z.update(sample(&[("A", 40.0), ("B", 60.0)]));
        assert_eq!(z.update(sample(&[("A", 45.0), ("B", 61.0)])), Some(61.0));
        assert_eq!(z.update(sample(&[])), None);
    }
}
