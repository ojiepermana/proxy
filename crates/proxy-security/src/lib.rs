//! Kebijakan akses berdasarkan alamat IP peer TCP.

use std::collections::HashSet;
use std::net::IpAddr;

/// Aturan IP persis. Denylist mendapat prioritas ketika sebuah IP ada di kedua daftar.
#[derive(Default)]
pub struct IpAccessPolicy {
    allowlist: HashSet<IpAddr>,
    denylist: HashSet<IpAddr>,
}

impl IpAccessPolicy {
    pub fn new(allowlist: Vec<IpAddr>, denylist: Vec<IpAddr>) -> Self {
        Self {
            allowlist: allowlist.into_iter().map(|ip| ip.to_canonical()).collect(),
            denylist: denylist.into_iter().map(|ip| ip.to_canonical()).collect(),
        }
    }

    pub fn allows(&self, peer_ip: Option<IpAddr>) -> bool {
        let Some(ip) = peer_ip.map(|ip| ip.to_canonical()) else {
            return self.allowlist.is_empty() && self.denylist.is_empty();
        };
        !self.denylist.contains(&ip) && (self.allowlist.is_empty() || self.allowlist.contains(&ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_empty_lists_allowlist_denylist_and_overlap() {
        let a = "192.0.2.10".parse().unwrap();
        let b = "2001:db8::2".parse().unwrap();
        for (policy, expected) in [
            (IpAccessPolicy::default(), [true, true, true]),
            (IpAccessPolicy::new(vec![a], vec![]), [true, false, false]),
            (IpAccessPolicy::new(vec![], vec![b]), [true, false, false]),
            (
                IpAccessPolicy::new(vec![a, b], vec![a]),
                [false, true, false],
            ),
        ] {
            assert_eq!(
                [Some(a), Some(b), None].map(|peer| policy.allows(peer)),
                expected
            );
        }
    }

    #[test]
    fn matches_ipv4_and_its_ipv6_mapped_form() {
        let ipv4 = "192.0.2.10".parse().unwrap();
        let mapped = "::ffff:192.0.2.10".parse().unwrap();
        assert!(IpAccessPolicy::new(vec![mapped], vec![]).allows(Some(ipv4)));
        assert!(!IpAccessPolicy::new(vec![mapped], vec![ipv4]).allows(Some(mapped)));
    }
}
