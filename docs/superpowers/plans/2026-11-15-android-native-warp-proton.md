# Android Native WARP WireGuard + All Chain Exits Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Android Usque supports selectable MASQUE and direct WARP WireGuard UDP, while every existing chain source (OpenVPN, WireGuard, WARP via WireGuard, Proton Guest, VPN Gate, HTTP, SOCKS5) works with either.

**Architecture:** Keep the existing MASQUE and Flutter design. Add an outer-underlay selection contract and a protected UDP WireGuard path that feeds the shared netstack and chain pipeline. Adapt the shared chain transport and Proton's control channel to consume the selected underlay rather than assuming MASQUE.

**Tech Stack:** Rust 1.97.1, Tokio, boringtun 0.7.1, Flutter 3.44.7, Kotlin Android VpnService, Android NDK 29.0.14206865, GitHub Actions.

**Spec:** docs/superpowers/specs/2026-11-15-android-dual-warp-proton.md

## Global Constraints

- Android first; keep the original Usque UI, branding and existing app ID.
- Keep MASQUE behavior as default and all existing chain sources intact.
- Direct WARP WireGuard is an outer UDP transport, not the existing MASQUE-over-WG chain source.
- Never leak traffic when either the WARP underlay or selected Proton chain becomes unavailable.
- Never log private keys, Proton auth tokens, verification URLs or raw credentials.
- No dependency installation, build, test, emulator or network probes outside GitHub Actions in ntun7719/Container.
- Source branch: ntun7729/usque-app:feature/android-native-warp-proton; main stays unchanged.

## Review Focus

- **No UDP route / blocked port:** native WARP connect fails clearly; MASQUE remains independently selectable. Test in Task 3.
- **VPN self-loop:** Android protects and binds the native UDP socket before sends; no packets escape into its own TUN. Test in Task 2.
- **Physical network handover:** stale socket generation cannot be used; connection and inner chain reauthorize. Test in Tasks 2 and 5.
- **Proton Guest expires/rejects authorization:** do not present connected or fall back to WARP-only. Test in Task 5.
- **Old configuration:** missing transport field defaults to MASQUE and existing chain settings are untouched. Test in Task 1 and Task 4.

---

### Task 1: Stable outer transport mode contract

**Files:** Create crates/usque-core/src/config/warp_underlay.rs; modify crates/usque-core/src/config/mod.rs; tests in warp_underlay.rs.

**Interfaces:** Produces public WarpUnderlay { Masque, NativeWireGuard }, serde values masque/native_wireguard, Default=Masque, as_str() -> &'static str. Does not change DataPlaneMode until the runtime is ready.

- [ ] Write tests for exact serde strings, invalid values, and default MASQUE.
- [ ] Run `cargo test -p usque-core warp_underlay --locked` in the Container workflow; expect tests to fail before implementation.
- [ ] Implement enum/serialization/default/method and re-export.
- [ ] Re-run `cargo test -p usque-core warp_underlay --locked` and `cargo fmt --all --check` in Container; expect PASS.
- [ ] Commit the implementation separately on the feature branch.

### Task 2: Protected physical UDP socket boundary

**Files:** Create crates/usque-transport/src/native_warp_udp.rs, tests in native_warp_udp.rs; modify crates/usque-transport/src/lib.rs only for integration.

**Interfaces:** Socket acquisition consumes an endpoint SocketAddr, Arc<dyn SocketProtector>, expected generation u64 and CancellationToken; returns a protected Tokio UdpSocket with a held DirectEgressLease. All outgoing native WG packets use this handle. Do not bypass SocketProtector.

- [ ] Write tests for IPv4/IPv6 endpoint validation, zero/invalid port, protect refusal, generation advance, cancellation and bounded datagrams.
- [ ] Run focused Rust tests in Container; expect FAIL initially.
- [ ] Implement binding, protector generation check, UDP connect, and bounded receive/send with generation-scoped resource ownership.
- [ ] Run focused tests and Android target clippy in Container; expect PASS.
- [ ] Commit and review socket ownership and fd protection before proceeding.

### Task 3: Native WARP WireGuard underlay actor

**Files:** Create crates/usque-transport/src/native_warp_wireguard.rs; reuse crates/usque-transport/src/wireguard.rs; modify crates/usque-transport/src/lib.rs.

**Interfaces:** Start with a validated Warp WG WireGuardProfile and protected UDP handle; feed encrypted packets to wireguard::Input kind 1 and plain IP to kind 2; consume Network/State/IpPacket/TransportPacket events with generation equality. Stop cancels actor and closes socket.

- [ ] Write deterministic tests: handshake initiation, encrypted/inbound dispatch, payload caps, stale generation, no UDP connect, timeout, cancellation and reconnection.
- [ ] Run focused tests in Container; expect FAIL initially.
- [ ] Implement a bounded worker wrapping the existing boringtun actor; do not duplicate key management or cryptography.
- [ ] Run focused tests, fmt and Android clippy; expect PASS.
- [ ] Commit the native underlay actor without enabling it as user-facing functionality.

### Task 4: Persist selection and bridge Android VPN/TUN

**Files:** Modify crates/usque-core/src/config/mod.rs; crates/usque-core/src/config/data_plane.rs; crates/usque-transport/src/data_plane.rs; crates/usque-android/src/lib.rs; Android native service/profile adapter files under apps/usque_gui/android/app/src/main/kotlin/io/github/georgexie2333/usque/.

**Interfaces:** Add a backward-compatible, default-MASQUE outer-transport choice to profile storage and Android JSON parsing; DataPlaneRuntime acquires a native WG underlay when selected. Retain L4Proxy as a separate data-plane mode and existing MASQUE logic. `internal_network`, `send_owned_packet`, `attach_tun`, DNS and frontends retain their current contracts.

- [ ] Write migration/round-trip tests for existing profiles and explicit native WG selection.
- [ ] Run focused Rust/Android host tests in Container; expect FAIL initially.
- [ ] Wire protected native UDP underlay through shared packet frontends, routing and lifecycle; refuse unavailable configuration instead of falling back.
- [ ] Run workspace Rust tests, Android host tests and APK compile in Container; expect PASS.
- [ ] Commit only once MASQUE regression checks and Android TUN protection tests pass.

### Task 5: All seven chain modes across both WARP transports

**Files:** Modify crates/usque-transport/src/data_plane.rs; crates/usque-engine/src/proton_vpn.rs; crates/usque-engine/src/chain_exit.rs; crates/usque-android/src/lib.rs as needed.

**Interfaces:** Preserve all seven chain sources via the same outer transport interface: OpenVPN custom, WireGuard custom, WARP via WireGuard, Proton Guest, VPN Gate, HTTP CONNECT, SOCKS5. Existing Proton Guest account/session/auth/certificate logic stays intact. Resolve chain routing, authorization, DNS, and endpoint socket semantics through the selected protected underlay; no selected chain is silently dropped.

- [ ] Write a 2x7 underlay/chain-source matrix covering both dispatch modes; include guest account reuse, certificate refresh, OpenVPN UDP/TCP, custom WireGuard, WARP WireGuard, VPN Gate, HTTP CONNECT, SOCKS5, stale session, and no fallback when selected chain authorization fails.
- [ ] Run focused tests in Container; expect FAIL initially.
- [ ] Refactor MASQUE-only chain transport assumptions into an outer-underlay contract consumed by every exit, preserving all existing import, authentication and certificate code.
- [ ] Run Rust/Android CI and verify other chain sources still work; expect PASS.
- [ ] Commit chain integration only after all seven sources have deterministic coverage; document live-test gaps separately.

### Task 6: Original Flutter UI — transport selector

**Files:** Modify apps/usque_gui/lib/models/app_models.dart, existing connection/settings widgets, services/control_codec.dart, Android model/field adapter as needed; add Flutter tests in apps/usque_gui/test/.

**Interfaces:** Show MASQUE vs WireGuard mode without redesign, retain H3/H2 MASQUE selector, render configured WARP WireGuard profile and endpoint, preserve the Proton chain picker. Apply/reconnect remains explicit.

- [ ] Write widget/model tests for default MASQUE, persisted selection, Proton Guest + native WG and invalid configuration.
- [ ] Run Flutter widget tests in Container; expect FAIL initially.
- [ ] Implement the selector, validation and save/apply wiring.
- [ ] Run Flutter analyzer/tests and APK compile in Container; expect PASS.
- [ ] Commit preserving original fonts, layout, navigation and brand assets.

### Task 7: Android real-device acceptance and APK

**Files:** Add docs/ANDROID_DUAL_WARP_VALIDATION.md and required Android host tests; use Container workflow .github/workflows/usque-android-native.yml.

**Interfaces:** CI uploads a separately installable Android arm64 APK; live test matrix must distinguish verified results from not executed.

- [ ] Run Rust fmt/test/clippy, Flutter analyze/tests, Kotlin/Android tests and APK compile in Container.
- [ ] Validate APK signature, package ID, ABI and bundled native libraries via existing repo verification tools.
- [ ] Perform authorized-device test matrix: MASQUE, WG, Proton Guest with both, DNS and IPv6, kill switch, SOCKS5/HTTP, per-app bypass and network handover.
- [ ] Record each result as PASS / FAIL / NOT RUN; never infer live connectivity from unit tests or APK compilation.
- [ ] Open draft PR; merge only after required tests and device checks pass.

## CI workspace contract

- `ntun7719/Container` hosts and runs all workflows. Checkout `ntun7729/usque-app` feature branch with `persist-credentials: false` and no secrets accessible to fork code.
- PIN the fork commit SHA for reproducibility where possible; record commit and rust/flutter/android toolchain versions in logs.
- Default scope is targeted Rust tests. Run full native Android builds only for integration milestones; publish APK through actions/upload-artifact.
- A GitHub Actions pass proves compilation and deterministic tests; it does not prove the WARP or Proton network path works on a real Android phone.
