//! Protected native UDP underlay for direct WARP WireGuard.
//!
//! Unlike a WireGuard chain exit, this socket is the physical WARP underlay.
//! Android must protect the socket before it can contact the remote endpoint.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;

use thiserror::Error;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use crate::socket::{
    DirectEgressLease, DirectProtocol, STALE_GENERATION_REASON, SocketProtector, socket_handle,
};

/// Native WARP UDP must fit the transport's bounded receive slot.
pub const MAX_NATIVE_WARP_DATAGRAM: usize = crate::udp_io::UDP_RECEIVE_SLOT_SIZE;

#[derive(Debug, Error)]
pub enum NativeWarpUdpError {
    #[error("native WARP endpoint is invalid")]
    InvalidEndpoint,
    #[error("physical socket protection failed: {0}")]
    Protection(String),
    #[error("physical network generation changed during native WARP I/O")]
    StaleGeneration,
    #[error("native WARP UDP packet exceeds the bounded receive slot")]
    PacketTooLarge,
    #[error("native WARP UDP operation was cancelled")]
    Cancelled,
    #[error("native WARP UDP socket I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

/// A protected, generation-scoped UDP connection to one WARP WireGuard peer.
///
/// Field order is significant: the kernel socket is closed before the
/// platform's egress authorization lease is released.
pub struct NativeWarpUdp {
    socket: UdpSocket,
    _egress_lease: DirectEgressLease,
    protector: Arc<dyn SocketProtector>,
    expected_generation: u64,
    cancel: CancellationToken,
}

impl NativeWarpUdp {
    /// Binds, protects, and connects a new physical UDP socket without sending
    /// a datagram. The caller must re-create this handle after network changes.
    pub async fn connect(
        remote: SocketAddr,
        protector: Arc<dyn SocketProtector>,
        expected_generation: u64,
        cancel: CancellationToken,
    ) -> Result<Self, NativeWarpUdpError> {
        if remote.port() == 0
            || remote.ip().is_unspecified()
            || remote.ip().is_multicast()
            || remote.ip() == IpAddr::V4(Ipv4Addr::BROADCAST)
        {
            return Err(NativeWarpUdpError::InvalidEndpoint);
        }
        if cancel.is_cancelled() {
            return Err(NativeWarpUdpError::Cancelled);
        }
        if protector.network_generation().unwrap_or_default() != expected_generation {
            return Err(NativeWarpUdpError::StaleGeneration);
        }

        let wildcard = SocketAddr::new(
            if remote.is_ipv4() {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            } else {
                IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            },
            0,
        );
        let socket = StdUdpSocket::bind(wildcard)?;
        socket.set_nonblocking(true)?;

        // Android's VpnService.protect(fd) and physical-network binding happen
        // here, before any network connect or packet send.
        let lease = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(NativeWarpUdpError::Cancelled),
            protected = protector.protect_for_target_generation(
                socket_handle(&socket),
                remote,
                DirectProtocol::Udp,
                expected_generation,
            ) => protected.map_err(|reason| {
                if reason == STALE_GENERATION_REASON {
                    NativeWarpUdpError::StaleGeneration
                } else {
                    NativeWarpUdpError::Protection(reason)
                }
            })?,
        };

        if cancel.is_cancelled() {
            return Err(NativeWarpUdpError::Cancelled);
        }
        if protector.network_generation().unwrap_or_default() != expected_generation
            || lease.generation() != Some(expected_generation)
        {
            return Err(NativeWarpUdpError::StaleGeneration);
        }
        socket.connect(remote)?;
        let socket = UdpSocket::from_std(socket)?;
        if protector.network_generation().unwrap_or_default() != expected_generation {
            return Err(NativeWarpUdpError::StaleGeneration);
        }

        Ok(Self {
            socket,
            _egress_lease: lease,
            protector,
            expected_generation,
            cancel,
        })
    }

    fn check_live(&self) -> Result<(), NativeWarpUdpError> {
        if self.cancel.is_cancelled() {
            return Err(NativeWarpUdpError::Cancelled);
        }
        if self.protector.network_generation().unwrap_or_default() != self.expected_generation {
            return Err(NativeWarpUdpError::StaleGeneration);
        }
        Ok(())
    }

    /// Sends one encrypted WireGuard datagram to the already-validated peer.
    pub async fn send(&self, packet: &[u8]) -> Result<(), NativeWarpUdpError> {
        if packet.len() > MAX_NATIVE_WARP_DATAGRAM {
            return Err(NativeWarpUdpError::PacketTooLarge);
        }
        self.check_live()?;
        let bytes = tokio::select! {
            biased;
            _ = self.cancel.cancelled() => return Err(NativeWarpUdpError::Cancelled),
            result = self.socket.send(packet) => result?,
        };
        if bytes != packet.len() {
            return Err(NativeWarpUdpError::Io(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "native WARP datagram was not sent completely",
            )));
        }
        self.check_live()
    }

    /// Receives exactly one complete WireGuard datagram and rejects truncation.
    pub async fn receive(&self) -> Result<Vec<u8>, NativeWarpUdpError> {
        self.check_live()?;
        let mut buffer = [0u8; MAX_NATIVE_WARP_DATAGRAM + 1];
        let count = tokio::select! {
            biased;
            _ = self.cancel.cancelled() => return Err(NativeWarpUdpError::Cancelled),
            result = self.socket.recv(&mut buffer) => result?,
        };
        self.check_live()?;
        if count > MAX_NATIVE_WARP_DATAGRAM {
            return Err(NativeWarpUdpError::PacketTooLarge);
        }
        Ok(buffer[..count].to_vec())
    }

    /// Cancels any in-progress operation; dropping this object closes the
    /// socket before its physical egress lease is released.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::{NoopSocketProtector, SocketHandle, SocketProtector};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
                NativeWarpUdp::connect(remote, Arc::clone(&protector), 0, CancellationToken::new())
                    .await,
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
        )
        .await
        .unwrap();
        assert_eq!(protector.count.load(Ordering::SeqCst), 1);
        link.send(b"wg-initial").await.unwrap();
        let mut buffer = [0_u8; 128];
        let (length, _) = peer.recv_from(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..length], b"wg-initial");
    }

    #[tokio::test]
    async fn native_warp_udp_denies_protector_rejection() {
        let p = Arc::new(TestProtector {
            fail: true,
            ..TestProtector::default()
        });
        assert!(matches!(
            NativeWarpUdp::connect(
                SocketAddr::from(([127, 0, 0, 1], 443)),
                p.clone(),
                0,
                CancellationToken::new()
            )
            .await,
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
        )
        .await
        .unwrap();
        p.generation.store(8, Ordering::SeqCst);
        assert!(matches!(
            link.send(b"x").await,
            Err(NativeWarpUdpError::StaleGeneration)
        ));
    }

    #[tokio::test]
    async fn native_warp_udp_refuses_oversized_datagrams() {
        let p: Arc<dyn SocketProtector> = Arc::new(NoopSocketProtector);
        let link = NativeWarpUdp::connect(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
            p,
            0,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(matches!(
            link.send(&vec![0; 2049]).await,
            Err(NativeWarpUdpError::PacketTooLarge)
        ));
    }
}
