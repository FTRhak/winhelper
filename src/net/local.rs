//! LAN IPv4 address via `GetAdaptersAddresses` (PLAN.md §5.4).

use std::net::Ipv4Addr;

use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST,
    GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST,
    IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_UNICAST_ADDRESS_LH,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN, SOCKET_ADDRESS};

/// The parts of an adapter that decide which address to show.
#[derive(Clone, Debug, Default)]
struct Adapter {
    up: bool,
    loopback: bool,
    has_gateway: bool,
    metric: u32,
    ipv4: Vec<Ipv4Addr>,
}

/// The LAN IPv4 of the adapter that carries traffic, as text.
pub fn lan_ip() -> Option<String> {
    choose(&read_adapters()?).map(|ip| ip.to_string())
}

/// Picks the first usable address of the best adapter: up, not loopback,
/// preferring one with a default gateway, then the lowest IPv4 route metric.
fn choose(adapters: &[Adapter]) -> Option<Ipv4Addr> {
    adapters
        .iter()
        .filter(|a| a.up && !a.loopback)
        .filter_map(|a| {
            let ip = a.ipv4.iter().copied().find(|ip| usable(*ip))?;
            Some(((!a.has_gateway, a.metric), ip))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, ip)| ip)
}

/// Excludes loopback, unspecified and APIPA (169.254/16, "no DHCP") addresses.
fn usable(ip: Ipv4Addr) -> bool {
    !(ip.is_loopback() || ip.is_unspecified() || ip.is_link_local())
}

fn read_adapters() -> Option<Vec<Adapter>> {
    let flags = GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_DNS_SERVER
        | GAA_FLAG_SKIP_FRIENDLY_NAME
        | GAA_FLAG_INCLUDE_GATEWAYS;
    let mut size: u32 = 16 * 1024;
    // The adapter list can grow between the size query and the call; retry a few times.
    for _ in 0..3 {
        // u64 elements keep the buffer 8-byte aligned for the structs inside.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let first = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `buf` holds `size` writable bytes, as the call is told.
        let ret =
            unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(first), &mut size) };
        if ret == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if ret != NO_ERROR.0 {
            return None;
        }
        // SAFETY: on success the buffer holds a valid linked list that lives as long as `buf`.
        return Some(unsafe { walk(first) });
    }
    None
}

/// # Safety
/// `first` must be the head of a list filled by `GetAdaptersAddresses`.
unsafe fn walk(first: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<Adapter> {
    let mut adapters = Vec::new();
    let mut node = first;
    while let Some(a) = node.as_ref() {
        let mut ipv4 = Vec::new();
        let mut unicast = a.FirstUnicastAddress as *const IP_ADAPTER_UNICAST_ADDRESS_LH;
        while let Some(u) = unicast.as_ref() {
            ipv4.extend(ipv4_of(&u.Address));
            unicast = u.Next;
        }
        adapters.push(Adapter {
            up: a.OperStatus == IfOperStatusUp,
            loopback: a.IfType == IF_TYPE_SOFTWARE_LOOPBACK,
            has_gateway: !a.FirstGatewayAddress.is_null(),
            metric: a.Ipv4Metric,
            ipv4,
        });
        node = a.Next;
    }
    adapters
}

/// # Safety
/// `addr` must point to a valid socket address of the stated length (or be null).
unsafe fn ipv4_of(addr: &SOCKET_ADDRESS) -> Option<Ipv4Addr> {
    let sa = addr.lpSockaddr as *const SOCKADDR_IN;
    if sa.is_null() || (addr.iSockaddrLength as usize) < std::mem::size_of::<SOCKADDR_IN>() {
        return None;
    }
    let sa = &*sa;
    // S_addr is stored in network byte order, i.e. its bytes are the octets.
    (sa.sin_family == AF_INET).then(|| Ipv4Addr::from(sa.sin_addr.S_un.S_addr.to_ne_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(gateway: bool, metric: u32, ips: &[[u8; 4]]) -> Adapter {
        Adapter {
            up: true,
            loopback: false,
            has_gateway: gateway,
            metric,
            ipv4: ips.iter().map(|o| Ipv4Addr::from(*o)).collect(),
        }
    }

    #[test]
    fn prefers_gateway_then_lowest_metric() {
        let hyper_v = adapter(false, 5, &[[172, 20, 0, 1]]);
        let wifi = adapter(true, 50, &[[192, 168, 1, 15]]);
        let ethernet = adapter(true, 25, &[[10, 0, 0, 7]]);
        assert_eq!(
            choose(&[hyper_v.clone(), wifi.clone(), ethernet]),
            Some(Ipv4Addr::new(10, 0, 0, 7))
        );
        assert_eq!(
            choose(&[hyper_v.clone(), wifi]),
            Some(Ipv4Addr::new(192, 168, 1, 15))
        );
        // Without any gateway, still show something.
        assert_eq!(choose(&[hyper_v]), Some(Ipv4Addr::new(172, 20, 0, 1)));
    }

    #[test]
    fn skips_down_loopback_and_unusable_addresses() {
        let down = Adapter {
            up: false,
            ..adapter(true, 1, &[[192, 168, 0, 2]])
        };
        let loopback = Adapter {
            loopback: true,
            ..adapter(true, 1, &[[127, 0, 0, 1]])
        };
        let apipa_then_real = adapter(true, 30, &[[169, 254, 3, 4], [192, 168, 1, 15]]);
        let apipa_only = adapter(true, 2, &[[169, 254, 9, 9]]);
        assert_eq!(
            choose(&[down, loopback, apipa_only.clone(), apipa_then_real]),
            Some(Ipv4Addr::new(192, 168, 1, 15))
        );
        assert_eq!(choose(&[apipa_only]), None);
        assert_eq!(choose(&[]), None);
    }
}
