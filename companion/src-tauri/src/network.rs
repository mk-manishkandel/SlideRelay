//! Picks the computer's address on the classroom network (the one students'
//! phones can reach), skipping VPNs, virtual machines and other adapters
//! that phones can't see.

use std::net::Ipv4Addr;

/// One network adapter address, as reported by the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    pub name: String,
    pub ip: Ipv4Addr,
}

/// Lists the computer's IPv4 adapters.
pub fn adapters() -> Vec<Adapter> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|iface| match iface.ip() {
            std::net::IpAddr::V4(ip) => Some(Adapter {
                name: iface.name.clone(),
                ip,
            }),
            _ => None,
        })
        .collect()
}

/// Adapter names that are never the classroom network: short Unix-style
/// prefixes ("utun3") and words found in longer Windows names.
const VIRTUAL_PREFIXES: &[&str] = &[
    "utun", "tun", "tap", "wg", "awdl", "llw", "anpi", "ppp", "bridge", "docker", "veth",
];
const VIRTUAL_WORDS: &[&str] = &[
    "vethernet",
    "virtualbox",
    "vmware",
    "vmnet",
    "hyper-v",
    "docker",
    "tailscale",
    "zerotier",
    "vpn",
    "loopback",
    "bluetooth",
];

/// Adapter names that are usually the real Wi-Fi or Ethernet.
const PREFERRED_HINTS: &[&str] = &[
    "en0", "en1", "wi-fi", "wifi", "wlan", "wireless", "ethernet",
];

fn is_private(ip: Ipv4Addr) -> bool {
    ip.is_private() // 10/8, 172.16/12, 192.168/16
}

/// Higher score = more likely to be the classroom network. `None` = unusable.
fn score(adapter: &Adapter) -> Option<i32> {
    let ip = adapter.ip;
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
        return None;
    }
    let name = adapter.name.to_lowercase();
    if VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
        || VIRTUAL_WORDS.iter().any(|w| name.contains(w))
    {
        return None;
    }
    // 100.64/10 is carrier-grade NAT, used by Tailscale and some VPNs.
    if ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]) {
        return None;
    }
    let mut s = 0;
    if is_private(ip) {
        s += 10;
    }
    // Home/school routers mostly hand out 192.168.x.x or 10.x.x.x.
    if ip.octets()[0] == 192 && ip.octets()[1] == 168 {
        s += 2;
    }
    if PREFERRED_HINTS
        .iter()
        .any(|h| name == *h || name.starts_with(h))
    {
        s += 5;
    }
    Some(s)
}

/// Chooses the best address for students to connect to.
/// `preferred` (from settings) wins if that address still exists.
pub fn choose_lan_ip(adapters: &[Adapter], preferred: Option<&str>) -> Option<Ipv4Addr> {
    if let Some(p) = preferred.and_then(|p| p.parse::<Ipv4Addr>().ok()) {
        if adapters.iter().any(|a| a.ip == p) {
            return Some(p);
        }
    }
    adapters
        .iter()
        .filter_map(|a| score(a).map(|s| (s, a.ip)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, ip)| ip)
}

/// Adapters worth offering in Settings (usable ones, best first).
pub fn usable_adapters(adapters: &[Adapter]) -> Vec<Adapter> {
    let mut list: Vec<(i32, Adapter)> = adapters
        .iter()
        .filter_map(|a| score(a).map(|s| (s, a.clone())))
        .collect();
    list.sort_by_key(|(s, _)| -s);
    list.into_iter().map(|(_, a)| a).collect()
}

/// "http://192.168.1.20" (port 80 is left out so the link is shorter).
pub fn base_url(ip: Ipv4Addr, port: u16) -> String {
    if port == 80 {
        format!("http://{ip}")
    } else {
        format!("http://{ip}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(name: &str, ip: &str) -> Adapter {
        Adapter {
            name: name.into(),
            ip: ip.parse().unwrap(),
        }
    }

    #[test]
    fn picks_wifi_over_vpn_and_virtual() {
        let list = vec![
            a("lo0", "127.0.0.1"),
            a("utun3", "10.8.0.2"),
            a("bridge100", "192.168.64.1"),
            a("en0", "192.168.1.23"),
            a("vEthernet (WSL)", "172.20.0.1"),
        ];
        assert_eq!(
            choose_lan_ip(&list, None),
            Some("192.168.1.23".parse().unwrap())
        );
    }

    #[test]
    fn windows_names() {
        let list = vec![
            a("VirtualBox Host-Only Network", "192.168.56.1"),
            a("Wi-Fi", "10.0.0.15"),
            a("Ethernet 2", "169.254.10.3"),
        ];
        assert_eq!(
            choose_lan_ip(&list, None),
            Some("10.0.0.15".parse().unwrap())
        );
    }

    #[test]
    fn skips_cgnat_vpn_ranges() {
        let list = vec![a("tailscale0", "100.101.5.6"), a("eth0", "100.80.1.1")];
        assert_eq!(choose_lan_ip(&list, None), None);
    }

    #[test]
    fn teacher_choice_wins_if_present() {
        let list = vec![a("en0", "192.168.1.23"), a("en5", "10.1.2.3")];
        assert_eq!(
            choose_lan_ip(&list, Some("10.1.2.3")),
            Some("10.1.2.3".parse().unwrap())
        );
        // Gone (e.g. unplugged): fall back to automatic.
        assert_eq!(
            choose_lan_ip(&list, Some("10.9.9.9")),
            Some("192.168.1.23".parse().unwrap())
        );
    }

    #[test]
    fn base_url_hides_port_80() {
        let ip = "192.168.1.5".parse().unwrap();
        assert_eq!(base_url(ip, 80), "http://192.168.1.5");
        assert_eq!(base_url(ip, 8080), "http://192.168.1.5:8080");
    }
}
