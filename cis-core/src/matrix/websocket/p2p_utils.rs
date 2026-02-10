//! # P2P Utilities for WebSocket Federation
//!
//! Provides utility functions for P2P networking, including LAN detection
//! and transport selection.

use std::net::{IpAddr, SocketAddr};

/// Check if an IP address is in a private/local network range
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 10.0.0.0/8
            if octets[0] == 10 {
                return true;
            }
            // 172.16.0.0/12
            if octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31 {
                return true;
            }
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            // 127.0.0.0/8 (loopback)
            if octets[0] == 127 {
                return true;
            }
            // 169.254.0.0/16 (link-local)
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            false
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // ::1 (loopback)
            if segments == [0, 0, 0, 0, 0, 0, 0, 1] {
                return true;
            }
            // fc00::/7 (unique local addresses)
            if (segments[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            // fe80::/10 (link-local)
            if (segments[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            false
        }
    }
}

/// Check if two socket addresses are on the same LAN
///
/// This function compares the network portions of the addresses to determine
/// if they are likely on the same local network.
///
/// # Arguments
/// * `local` - The local socket address
/// * `remote` - The remote socket address
///
/// # Returns
/// `true` if the addresses appear to be on the same LAN
pub fn is_same_lan(local: SocketAddr, remote: SocketAddr) -> bool {
    // Both must be IP addresses of the same version
    match (local.ip(), remote.ip()) {
        (IpAddr::V4(local_ip), IpAddr::V4(remote_ip)) => {
            // Both must be private/local IPs
            if !is_private_ip(IpAddr::V4(local_ip)) || !is_private_ip(IpAddr::V4(remote_ip)) {
                return false;
            }

            let local_octets = local_ip.octets();
            let remote_octets = remote_ip.octets();

            // Check if in the same /24 subnet (typical for home/small office networks)
            local_octets[0] == remote_octets[0]
                && local_octets[1] == remote_octets[1]
                && local_octets[2] == remote_octets[2]
        }
        (IpAddr::V6(local_ip), IpAddr::V6(remote_ip)) => {
            // Both must be local IPv6 addresses
            if !is_private_ip(IpAddr::V6(local_ip)) || !is_private_ip(IpAddr::V6(remote_ip)) {
                return false;
            }

            let local_segments = local_ip.segments();
            let remote_segments = remote_ip.segments();

            // For ULA (fc00::/7), compare first 4 segments (64-bit prefix)
            // For link-local (fe80::/10), they are on the same link
            if (local_segments[0] & 0xffc0) == 0xfe80 {
                // Both are link-local, assume same link
                true
            } else {
                // Compare /64 prefix for ULA
                local_segments[0] == remote_segments[0]
                    && local_segments[1] == remote_segments[1]
                    && local_segments[2] == remote_segments[2]
                    && local_segments[3] == remote_segments[3]
            }
        }
        // Different IP versions cannot be on the same LAN
        _ => false,
    }
}

/// Get the local socket address that would be used to connect to a remote address
///
/// This is a helper function that returns a best-effort local address.
/// In practice, the actual local address depends on the routing table.
///
/// # Arguments
/// * `remote` - The remote socket address to connect to
///
/// # Returns
/// An optional local socket address (returns None if unable to determine)
pub fn get_local_address_for(remote: SocketAddr) -> Option<SocketAddr> {
    // For private IP ranges, we can make educated guesses about the local address
    match remote.ip() {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            
            // 10.0.0.0/8
            if octets[0] == 10 {
                return Some(SocketAddr::new(
                    IpAddr::V4(std::net::Ipv4Addr::new(10, octets[1], octets[2], 1)),
                    0,
                ));
            }
            // 172.16.0.0/12
            if octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31 {
                return Some(SocketAddr::new(
                    IpAddr::V4(std::net::Ipv4Addr::new(172, octets[1], octets[2], 1)),
                    0,
                ));
            }
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                return Some(SocketAddr::new(
                    IpAddr::V4(std::net::Ipv4Addr::new(192, 168, octets[2], 1)),
                    0,
                ));
            }
            // 127.0.0.0/8 (loopback)
            if octets[0] == 127 {
                return Some(SocketAddr::new(
                    IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    0,
                ));
            }
            // 169.254.0.0/16 (link-local)
            if octets[0] == 169 && octets[1] == 254 {
                return Some(SocketAddr::new(
                    IpAddr::V4(std::net::Ipv4Addr::new(169, 254, octets[2], 1)),
                    0,
                ));
            }
            None
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            
            // ::1 (loopback)
            if segments == [0, 0, 0, 0, 0, 0, 0, 1] {
                return Some(SocketAddr::new(
                    IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    0,
                ));
            }
            // fc00::/7 (ULA)
            if (segments[0] & 0xfe00) == 0xfc00 {
                return Some(SocketAddr::new(
                    IpAddr::V6(std::net::Ipv6Addr::new(
                        segments[0], segments[1], segments[2], segments[3], 0, 0, 0, 1,
                    )),
                    0,
                ));
            }
            // fe80::/10 (link-local)
            if (segments[0] & 0xffc0) == 0xfe80 {
                return Some(SocketAddr::new(
                    IpAddr::V6(std::net::Ipv6Addr::new(
                        segments[0], segments[1], segments[2], segments[3], 0, 0, 0, 1,
                    )),
                    0,
                ));
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_ip_v4() {
        // Private ranges
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.255.255.255".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("172.31.255.255".parse().unwrap()));
        assert!(is_private_ip("192.168.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.255.255".parse().unwrap()));
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("169.254.1.1".parse().unwrap()));

        // Public ranges
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip("172.32.0.1".parse().unwrap()));
        assert!(!is_private_ip("192.169.0.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6() {
        // Private ranges
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(is_private_ip("fc00::1".parse().unwrap()));
        assert!(is_private_ip("fd00::1".parse().unwrap()));
        assert!(is_private_ip("fe80::1".parse().unwrap()));

        // Public ranges
        assert!(!is_private_ip("2001:db8::1".parse().unwrap()));
        assert!(!is_private_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn test_is_same_lan_v4() {
        let local = SocketAddr::new("192.168.1.10".parse().unwrap(), 8080);
        let remote_same = SocketAddr::new("192.168.1.20".parse().unwrap(), 8080);
        let remote_diff = SocketAddr::new("192.168.2.20".parse().unwrap(), 8080);
        let remote_public = SocketAddr::new("8.8.8.8".parse().unwrap(), 8080);

        assert!(is_same_lan(local, remote_same));
        assert!(!is_same_lan(local, remote_diff));
        assert!(!is_same_lan(local, remote_public));
    }

    #[test]
    fn test_is_same_lan_v6() {
        let local = SocketAddr::new("fe80::1".parse().unwrap(), 8080);
        let remote_same = SocketAddr::new("fe80::2".parse().unwrap(), 8080);

        assert!(is_same_lan(local, remote_same));
    }

    #[test]
    fn test_is_same_lan_different_versions() {
        let local_v4 = SocketAddr::new("192.168.1.1".parse().unwrap(), 8080);
        let remote_v6 = SocketAddr::new("::1".parse().unwrap(), 8080);

        assert!(!is_same_lan(local_v4, remote_v6));
    }
}
