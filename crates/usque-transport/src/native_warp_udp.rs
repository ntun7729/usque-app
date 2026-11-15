//! Protected native UDP underlay for direct WARP WireGuard.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::{NoopSocketProtector, SocketHandle, SocketProtector};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    #[derive(Default)]
    struct TestProtector {
        count: AtomicUsize,
        generation: AtomicU64,
        fail: bool,
    }

    impl SocketProtector for TestProtector {
        fn protect(&self, _socket: SocketHandle) -> Result<(), String> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err("physical protection refused".to_owned())
            } else {
                Ok(())
            }
        }

        fn network_generation(&self) -> Option<u64> {
            Some(self.generation.load(Ordering::SeqCst))
        }
    }

    #[tokio::test]
    async fn native_warp_udp_rejects_unroutable_endpoints() {
        let protector: Arc<dyn SocketProtector> = Arc::new(NoopSocketProtector);
        for remote in [
            SocketAddr::from(([0, 0, 0, 0], 443)),
            SocketAddr::from(([224, 0, 0, 1], 443)),
            SocketAddr::from(([1, 1, 1, 1], 0)),
        ] {
            assert!(matches!(
                NativeWarpUdp::connect(remote, Arc::clone(&protector), 0, CancellationToken::new()).await,
                Err(NativeWarpUdpError::InvalidEndpoint)
            ));
        }
    }

    #[tokio::test]
    async fn native_warp_udp_protects_socket_before_sending() {
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let protector = Arc::new(TestProtector::default());
        let link = NativeWarpUdp::connect(
            peer.local_addr().unwrap(),
            protector.clone(),
            0,
            CancellationToken::new(),
        ).await.unwrap();
        assert_eq!(protector.count.load(Ordering::SeqCst), 1);
        link.send(b"wg-initial").await.unwrap();
        let mut buffer = [0_u8; 128];
        let (length, _) = peer.recv_from(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..length], b"wg-initial");
    }

    #[tokio::test]
    async fn native_warp_udp_denies_protector_rejection() {
        let p = Arc::new(TestProtector { fail: true, ..TestProtector::default() });
        assert!(matches!(
            NativeWarpUdp::connect(
                SocketAddr::from(([127, 0, 0, 1], 443)), p.clone(), 0, CancellationToken::new()
            ).await,
            Err(NativeWarpUdpError::Protection(_))
        ));
        assert_eq!(p.count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn native_warp_udp_fails_closed_on_generation_change() {
        let p = Arc::new(TestProtector::default());
        p.generation.store(7, Ordering::SeqCst);
        let link = NativeWarpUdp::connect(
            SocketAddr::from(([127, 0, 0, 1], 443)),
            p.clone(),
            7,
            CancellationToken::new(),
        ).await.unwrap();
        p.generation.store(8, Ordering::SeqCst);
        assert!(matches!(link.send(b"x").await, Err(NativeWarpUdpError::StaleGeneration)));
    }

    #[tokio::test]
    async fn native_warp_udp_refuses_oversized_datagrams() {
        let p: Arc<dyn SocketProtector> = Arc::new(NoopSocketProtector);
        let link = NativeWarpUdp::connect(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443), p, 0, CancellationToken::new()
        ).await.unwrap();
        assert!(matches!(
            link.send(&vec![0; 2049]).await,
            Err(NativeWarpUdpError::PacketTooLarge)
        ));
    }
}
