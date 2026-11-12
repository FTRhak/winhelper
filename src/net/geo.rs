//! Public IP + location providers and their reply parsing (PLAN.md §5.5).
//!
//! Providers are tried in order until one answers. The last one only returns
//! the IP, so at least that is shown when both geo services fail.

use std::net::IpAddr;

use serde_json::Value;

use super::http::Session;

/// Public IP and the approximate place it is registered to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Geo {
    pub ip: String,
    /// "City, Country", or whatever part the provider knows.
    pub location: Option<String>,
    /// ISO 3166 alpha-2, upper case.
    pub country_code: Option<String>,
}

struct Provider {
    host: &'static str,
    path: &'static str,
    parse: fn(&[u8]) -> Option<Geo>,
}

/// ipwho.is first: unlike ipinfo.io it returns the country name as well as the
/// code, which the "City, Country" row needs.
const PROVIDERS: [Provider; 3] = [
    Provider {
        host: "ipwho.is",
        path: "/",
        parse: parse_ipwhois,
    },
    Provider {
        host: "ipinfo.io",
        path: "/json",
        parse: parse_ipinfo,
    },
    Provider {
        host: "api.ipify.org",
        path: "/",
        parse: parse_ipify,
    },
];

/// Asks each provider in turn; `None` when all of them fail.
pub fn lookup(session: &Session) -> Option<Geo> {
    PROVIDERS.iter().find_map(|p| {
        let body = session.get(p.host, p.path).ok()?;
        (p.parse)(&body)
    })
}

/// `{"ip": "...", "success": true, "city": "Kyiv", "country": "Ukraine", "country_code": "UA", ...}`
fn parse_ipwhois(body: &[u8]) -> Option<Geo> {
    let v: Value = serde_json::from_slice(body).ok()?;
    if v.get("success").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    Some(Geo {
        ip: valid_ip(field(&v, "ip")?)?,
        location: location_text(field(&v, "city"), field(&v, "country")),
        country_code: country_code(field(&v, "country_code")),
    })
}

/// `{"ip": "...", "city": "Kyiv", "region": "Kyiv City", "country": "UA", ...}`.
/// `country` is only the code, so the location is just the city (or region).
fn parse_ipinfo(body: &[u8]) -> Option<Geo> {
    let v: Value = serde_json::from_slice(body).ok()?;
    Some(Geo {
        ip: valid_ip(field(&v, "ip")?)?,
        location: location_text(field(&v, "city").or(field(&v, "region")), None),
        country_code: country_code(field(&v, "country")),
    })
}

/// Plain-text IP.
fn parse_ipify(body: &[u8]) -> Option<Geo> {
    Some(Geo {
        ip: valid_ip(std::str::from_utf8(body).ok()?)?,
        ..Geo::default()
    })
}

/// Non-empty, trimmed string field.
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Accepts only a real IPv4 / IPv6 address, in canonical form.
fn valid_ip(s: &str) -> Option<String> {
    s.trim().parse::<IpAddr>().ok().map(|ip| ip.to_string())
}

fn location_text(city: Option<&str>, country: Option<&str>) -> Option<String> {
    match (city, country) {
        (Some(city), Some(country)) => Some(format!("{city}, {country}")),
        (one, other) => one.or(other).map(str::to_owned),
    }
}

fn country_code(s: Option<&str>) -> Option<String> {
    s.filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(str::to_ascii_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(ip: &str, location: Option<&str>, code: Option<&str>) -> Geo {
        Geo {
            ip: ip.into(),
            location: location.map(Into::into),
            country_code: code.map(Into::into),
        }
    }

    #[test]
    fn ipwhois_full_reply() {
        let body = br#"{"ip":"203.0.113.42","success":true,"type":"IPv4","city":"Kyiv",
            "region":"Kyiv City","country":"Ukraine","country_code":"UA",
            "connection":{"isp":"Example"}}"#;
        assert_eq!(
            parse_ipwhois(body),
            Some(geo("203.0.113.42", Some("Kyiv, Ukraine"), Some("UA")))
        );
    }

    #[test]
    fn ipwhois_failure_and_missing_fields() {
        let failed = br#"{"ip":"203.0.113.42","success":false,"message":"Reserved range"}"#;
        assert_eq!(parse_ipwhois(failed), None);
        let partial =
            br#"{"ip":"2001:db8::1","country":"Ukraine","city":"  ","country_code":"UKR"}"#;
        assert_eq!(
            parse_ipwhois(partial),
            Some(geo("2001:db8::1", Some("Ukraine"), None))
        );
        assert_eq!(parse_ipwhois(br#"{"success":true}"#), None);
        assert_eq!(parse_ipwhois(b"<html>rate limited</html>"), None);
    }

    #[test]
    fn ipinfo_reply() {
        let body = br#"{"ip":"203.0.113.42","city":"Kyiv","region":"Kyiv City",
            "country":"ua","loc":"50.4,30.5","org":"AS1 Example"}"#;
        assert_eq!(
            parse_ipinfo(body),
            Some(geo("203.0.113.42", Some("Kyiv"), Some("UA")))
        );
        let region_only = br#"{"ip":"203.0.113.42","region":"Kyiv City"}"#;
        assert_eq!(
            parse_ipinfo(region_only),
            Some(geo("203.0.113.42", Some("Kyiv City"), None))
        );
        assert_eq!(parse_ipinfo(br#"{"ip":"not an ip"}"#), None);
    }

    #[test]
    fn ipify_plain_text() {
        assert_eq!(
            parse_ipify(b"203.0.113.42\n"),
            Some(geo("203.0.113.42", None, None))
        );
        assert_eq!(parse_ipify(b"error"), None);
        assert_eq!(parse_ipify(b""), None);
    }

    #[test]
    fn location_combinations() {
        assert_eq!(location_text(Some("A"), Some("B")).as_deref(), Some("A, B"));
        assert_eq!(location_text(None, Some("B")).as_deref(), Some("B"));
        assert_eq!(location_text(Some("A"), None).as_deref(), Some("A"));
        assert_eq!(location_text(None, None), None);
    }
}
