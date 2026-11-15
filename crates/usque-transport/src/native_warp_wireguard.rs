//! Protected native WireGuard WARP packet actor.
//!
//! This carries WARP WireGuard packets directly over Android-protected UDP.
//! It does not silently enter the MASQUE path or bypass the final chain exit.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use usque_core::chain_exit::WireGuardProfile;
use usque_core::{AddressFamily, Transport};
use usque_openvpn::Event;

use crate::native_warp_udp::NativeWarpUdp;
use crate::netstack::{ExternalPacketChannels, ManagedTunnelRuntime, RuntimeHealth, RuntimePath};
use crate::packet_batch::PacketBatch;
use crate::socket::SocketProtector;
use crate::wireguard;
use crate::{NetworkQualityTelemetry, TransportError};

/// Outer UDP WireGuard WARP connection; separate from the chained WireGuard exit.
pub struct NativeWarpWireguard;

impl NativeWarpWireguard {
    /// Starts a protected physical WireGuard session and verifies the handshake
    /// before exposing a packet runtime to the shared Android TUN/proxy stack.
    pub async fn start(
        profile: WireGuardProfile,
        protector: Arc<dyn SocketProtector>,
        cancellation: CancellationToken,
    ) -> Result<(ManagedTunnelRuntime, (Ipv4Addr, Ipv6Addr)), TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::TunnelClosed);
        }

        let endpoint = profile.endpoint.address().ok_or_else(|| {
            TransportError::Netstack(
                "native WARP requires a pinned numeric WireGuard endpoint".into(),
            )
        })?;
        let _ipv4 = profile
            .addresses
            .iter()
            .find_map(|address| match address.addr() {
                IpAddr::V4(address) => Some(address),
                _ => None,
            })
            .filter(|address| !address.is_unspecified())
            .ok_or_else(|| {
                TransportError::Netstack("native WARP profile has no IPv4 assignment".into())
            })?;
        let ipv6 = profile
            .addresses
            .iter()
            .find_map(|address| match address.addr() {
                IpAddr::V6(address) => Some(address),
                _ => None,
            })
            .unwrap_or(Ipv6Addr::UNSPECIFIED);
        let generation = protector.network_generation().unwrap_or_default();
        let socket = NativeWarpUdp::connect(
            endpoint,
            Arc::clone(&protector),
            generation,
            cancellation.child_token(),
        )
        .await
        .map_err(|error| TransportError::Netstack(error.to_string()))?;

        let family = if endpoint.is_ipv4() {
            AddressFamily::Ipv4
        } else {
            AddressFamily::Ipv6
        };
        let path = RuntimePath {
            // The runtime carrier enum currently has MASQUE-only variants.
            // The outer mode remains independently serialized as native_wireguard.
            transport: Transport::Http3,
            endpoint_family: family,
            ipv4_available: true,
            ipv6_available: !ipv6.is_unspecified(),
        };
        let (mut tunnel, mut channels) =
            ManagedTunnelRuntime::for_external_packets(path, NetworkQualityTelemetry::default());
        channels.health.send_replace(RuntimeHealth::Connecting {
            requested: path,
            attempt: 1,
        });
        let network_path = tunnel.external_path();
        let (ready_tx, ready_rx) = oneshot::channel();
        let task_cancel = cancellation.child_token();
        let task = tokio::spawn(async move {
            let mut ready = Some(ready_tx);
            let task = drive(
                profile,
                socket,
                protector,
                generation,
                &mut channels,
                &mut ready,
                task_cancel.clone(),
                path,
                network_path,
            )
            .await;
            if let Err(error) = task {
                if !channels.cancellation.is_cancelled() && !task_cancel.is_cancelled() {
                    let message = error.to_string();
                    let failure = error.failure(Some(path.transport), Some(path.endpoint_family));
                    channels.failure.send_replace(Some(message.clone()));
                    channels.health.send_replace(RuntimeHealth::Failed {
                        last_path: path,
                        reconnect_count: 0,
                        message,
                        failure,
                    });
                }
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(error));
                }
            }
        });
        tunnel.track_external_task(task);

        let addresses = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(TransportError::TunnelClosed),
            result = tokio::time::timeout(Duration::from_secs(35), ready_rx) => {
                match result {
                    Ok(Ok(result)) => result?,
                    Ok(Err(_)) => return Err(TransportError::TunnelClosed),
                    Err(_) => return Err(TransportError::ConnectTimeout),
                }
            }
        };
        Ok((tunnel, addresses))
    }
}

enum PacketEvent {
    WireGuard(Result<Event, usque_openvpn::Error>),
    Encrypted(Result<Vec<u8>, crate::native_warp_udp::NativeWarpUdpError>),
    Plaintext(Option<crate::outbound_packet::OutboundPacket>),
    PhysicalCheck,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the one actor must preserve the physical lease, encrypted session, backpressure, readiness, and generation"
)]
async fn drive(
    profile: WireGuardProfile,
    socket: NativeWarpUdp,
    protector: Arc<dyn SocketProtector>,
    generation: u64,
    channels: &mut ExternalPacketChannels,
    ready: &mut Option<oneshot::Sender<Result<(Ipv4Addr, Ipv6Addr), TransportError>>>,
    cancellation: CancellationToken,
    path: RuntimePath,
    network_path: crate::netstack::ActiveNetworkPath,
) -> Result<(), TransportError> {
    let mut wireguard = wireguard::Session::start(profile);
    let input = wireguard.input();
    let mut active_generation = 1u64;
    let mut connected = false;
    let mut reconnect_count = 0u32;
    let mut outgoing = channels
        .outgoing
        .take()
        .ok_or(TransportError::TunnelClosed)?;
    let mut physical_tick = tokio::time::interval(Duration::from_secs(1));
    physical_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let event = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(TransportError::TunnelClosed),
            _ = channels.cancellation.cancelled() => return Err(TransportError::TunnelClosed),
            result = wireguard.next_event() => PacketEvent::WireGuard(result),
            result = socket.receive() => PacketEvent::Encrypted(result),
            result = outgoing.recv(), if connected => PacketEvent::Plaintext(result),
            _ = physical_tick.tick() => PacketEvent::PhysicalCheck,
        };

        match event {
            PacketEvent::PhysicalCheck => {
                if protector.network_generation().unwrap_or_default() != generation {
                    network_path.deactivate();
                    return Err(TransportError::TunnelClosed);
                }
            }
            PacketEvent::Encrypted(datagram) => {
                let datagram =
                    datagram.map_err(|error| TransportError::Netstack(error.to_string()))?;
                input
                    .push(1, active_generation, &datagram)
                    .await
                    .map_err(|_| TransportError::TunnelClosed)?;
            }
            PacketEvent::Plaintext(Some(packet)) => {
                let bytes = packet.freeze();
                input
                    .push_owned(2, active_generation, bytes.clone())
                    .await
                    .map_err(|_| TransportError::TunnelClosed)?;
                channels.counters.record_sent(bytes.len());
            }
            PacketEvent::Plaintext(None) => return Err(TransportError::TunnelClosed),
            PacketEvent::WireGuard(Err(_)) => return Err(TransportError::TunnelClosed),
            PacketEvent::WireGuard(Ok(event)) => match event {
                Event::Dial { generation } => {
                    active_generation = generation;
                    connected = false;
                    input
                        .push(3, generation, &[])
                        .await
                        .map_err(|_| TransportError::TunnelClosed)?;
                }
                Event::TransportPacket { generation, packet }
                    if generation == active_generation =>
                {
                    socket
                        .send(&packet)
                        .await
                        .map_err(|error| TransportError::Netstack(error.to_string()))?;
                }
                Event::IpPacket { generation, packet }
                    if generation == active_generation && connected =>
                {
                    let bytes = packet.len();
                    channels
                        .incoming
                        .send_observed(PacketBatch::single(packet), bytes)
                        .await
                        .map_err(|_| TransportError::TunnelClosed)?;
                    channels.counters.record_received(bytes);
                }
                Event::Network {
                    generation: packet_generation,
                    config,
                } if packet_generation == active_generation => {
                    let assigned = (
                        config.ipv4.ok_or_else(|| {
                            TransportError::Netstack("native WARP handshake omitted IPv4".into())
                        })?,
                        config.ipv6.unwrap_or(Ipv6Addr::UNSPECIFIED),
                    );
                    connected = true;
                    network_path.activate(Some(generation));
                    channels.health.send_replace(RuntimeHealth::Connected {
                        path,
                        reconnect_count,
                    });
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Ok(assigned));
                    }
                }
                Event::State {
                    generation,
                    name,
                    error,
                    fatal,
                } if generation == active_generation => {
                    if error || fatal {
                        network_path.deactivate();
                        return Err(TransportError::TunnelClosed);
                    }
                    if name == "RECONNECTING" {
                        connected = false;
                        reconnect_count = reconnect_count.saturating_add(1);
                        network_path.deactivate();
                        channels.health.send_replace(RuntimeHealth::Reconnecting {
                            last_path: path,
                            attempt: 1,
                            reconnect_count,
                            reason: "native_wireguard_handshake_restart".into(),
                            failure: TransportError::ConnectTimeout
                                .failure(Some(path.transport), Some(path.endpoint_family)),
                        });
                    }
                }
                _ => {}
            },
        }
    }
}

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
            endpoint: Endpoint {
                host: remote.ip().to_string(),
                port: remote.port(),
            },
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
            NativeWarpWireguard::start(config, Arc::new(crate::NoopSocketProtector), cancellation)
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
        let result =
            NativeWarpWireguard::start(config, Arc::new(crate::NoopSocketProtector), cancellation)
                .await;
        assert!(result.is_err());
    }
}
