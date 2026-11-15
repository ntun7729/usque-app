# Android dual WARP transport and Proton Guest — design

**Status:** Approved scope; implementation in progress. **Platform:** Android first. **Brand/UI:** keep original Usque design. **Chain priority:** preserve and validate ALL seven chain source types equally; Proton Guest is frequently used but is not privileged over other exits.

## Desired behavior

- Retain the current MASQUE CONNECT-IP (H3/H2) and L4 modes unchanged, including account profiles and DNS/routing policy.
- Add **native WARP WireGuard over UDP** as an explicit outer transport. It must not tunnel over MASQUE. A WireGuard chain exit is a different concept and remains supported.
- Continue to support VPN/TUN and HTTP/SOCKS5 proxy frontends, per-app exclusions, protected physical sockets, kill switch and existing routing.
- Preserve all seven chain-exit sources: OpenVPN, custom WireGuard, WARP via WireGuard, Proton VPN Guest, VPN Gate, HTTP CONNECT, SOCKS5. Every supported source must continue to work under both outer WARP modes, subject to verified UDP support where required.
- Permit: Application -> MASQUE WARP -> ANY existing supported chain exit -> Internet; Application -> native WG WARP -> SAME chain exit -> Internet; Application -> either WARP without chain.
- All existing chain configuration import, secrets, endpoint override, per-source DNS, and connect/reconnect behavior must remain unchanged. Proton Guest login, verification, server list, certificate renewal, encrypted credential storage and logical server selection retain their current user experience.
- Native WG transport selection must fail closed on invalid credentials, unreachable endpoints, connection loss, stale network generation and chain authorization failure. Never silently bypass Proton when a Proton chain is selected.
- Make no performance promises: handshake, successful DNS, end-to-end egress, reconnection, leak protection and throughput each require explicit validation.

## Existing architecture / constraints

- Rust 1.97.1 workspace; Flutter 3.44.7 pinned. Android NDK 29.0.14206865, API minimum 26.
- The existing core DataPlaneMode is ConnectIp | L4Proxy, and DataPlaneRuntime owns either MasqueRuntime or L4Runtime. This is not a native WireGuard transport yet.
- Existing wireguard.rs runs boringtun packet cryptography in memory; its network transport is currently supplied by the MASQUE chain.
- Android's AndroidSocketProtector and VpnService.protect must protect/bind the actual native WG UDP socket to the selected physical network before traffic starts.
- Existing WARP WireGuard profile generation depends on MASQUE. Support importing a compatible existing config first; remove registration dependency only after a validated direct registration path exists.
- Proton control traffic and its inner WireGuard chain presently use WARP-specific MASQUE assumptions; adapt through a shared WARP underlay contract, not separate Proton auth implementations.
- No changes to original branding or public application ID in initial stage. Experimental Android APKs should use a separate sideload application ID.

## Transport boundary

- Define a verified outer-WARP packet runtime with a single IP packet ingress/egress and shared network parameters, independent of MASQUE vs WireGuard.
- Implement native WG UDP using boringtun's existing Session, but put UDP send/receive and Android socket protection outside the cryptographic actor.
- Keep chain proxy above the chosen outer transport, with one shared session and DNS/routing policy. Never send chain control traffic outside the chosen underlay unexpectedly.
- Treat network change as a generation change: protect/rebind a fresh UDP socket, invalidate stale packets, renegotiate WG, then reauthorize the chain before allowing application data.
- Keep key material in existing encrypted sidecar; redact keys, credentials and raw Proton API responses from logs and diagnostic snapshots.

## Test and release acceptance

1. Deterministic Rust tests: native WG handshake timing, packet bounds and parsing, UDP send/receive (loopback), network-generation race, cancellation, shared outer transport interface.
2. Android host unit tests: VpnService.protect on native UDP sockets, start/stop lifecycle, network-change behavior, mode serialization and no routing leaks on errors.
3. Flutter tests: mode selection, persisted choices, Proton Guest selection, reconnect/apply behavior, no unnecessary redesign.
4. Android arm64 build and APK verification via GitHub Actions in ntun7719/Container. Do not run build/test commands in local sandbox.
5. Actual Android live tests (manual/authorized-device): MASQUE-only, WG-only, MASQUE+Proton guest, WG+Proton guest; DNS IPv4/IPv6 leakage, kill switch, per-app rules, SOCKS5 and HTTP, reconnect, UDP blocked.
6. Keep main branch untouched until CI and required device validation pass; document unverified paths rather than claiming success.

## Delivery milestones

- **M0:** Branch, plan, isolated CI runner targeting the fork.
- **M1:** Core transport contract and protected native WireGuard UDP transport tested independently.
- **M2:** Android native WARP selection integrated into Rust and Android service while MASQUE remains default.
- **M3:** All seven chain sources supported and tested over both outer transports, including Proton Guest.
- **M4:** Flutter mode selection in existing Usque design; Android experimental APK and live validation.
