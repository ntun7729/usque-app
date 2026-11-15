//! Direct WireGuard outer WARP actor. Unlike the existing chained exit,
//! packets here leave on a protected physical UDP socket.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::UdpSocket;
    use tokio::time::timeout;
    use tokio_util::sync::CancellationToken;
    use usque_core::chain_exit::{Endpoint, WireGuardProfile};
    use zeroize::Zeroizing;

    fn profile(remote: std::net::SocketAddr) -> WireGuardProfile {
        WireGuardProfile {
            private_key: Zeroizing::new([0x11_u8; 32]),
            public_key: [0x22_u8; 32],
            preshared_key: None,
            endpoint: Endpoint::parse(&remote.ip().to_string(), &remote.port().to_string(), 0)
                .unwrap(),
            addresses: vec!["172.16.0.2/32".parse().unwrap()],
            dns_servers: vec!["1.1.1.1".parse().unwrap()],
            allowed_ips: vec!["0.0.0.0/0".parse().unwrap()],
            mtu: 1280,
            keepalive: Some(25),
        }
    }

    #[tokio::test]
    async fn native_warp_wireguard_starts_real_udp_handshake() {
        let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let config = profile(peer.local_addr().unwrap());
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(async move {
            NativeWarpWireguard::start(
                config,
                Arc::new(crate::NoopSocketProtector),
                cancellation,
            )
            .await
        });
        let mut datagram = [0u8; 2048];
        let (received, _) = timeout(Duration::from_secs(3), peer.recv_from(&mut datagram))
            .await
            .expect("a native WireGuard handshake must leave on UDP")
            .unwrap();
        assert!(received >= 4);
        assert_eq!(&datagram[..4], &[1, 0, 0, 0]);
        task.abort();
    }

    #[tokio::test]
    async fn native_warp_wireguard_requires_a_valid_protected_endpoint() {
        let config = profile(std::net::SocketAddr::from(([127, 0, 0, 1], 443)));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result = NativeWarpWireguard::start(
            config,
            Arc::new(crate::NoopSocketProtector),
            cancellation,
        )
        .await;
        assert!(result.is_err());
    }
}
