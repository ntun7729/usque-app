# Android dual WARP validation

## Scope and evidence standards

This document tracks **direct WARP WireGuard UDP as an outer transport**, not
the older `WARP via WireGuard` **chain-exit** feature. The same seven chain-exit
sources must work with MASQUE and with native WireGuard. A clean GitHub Actions
build establishes compilation and deterministic test results only. It does
**not** establish that UDP is reachable in the user's network or that an
on-device WARP/chain session is usable.

All live-device entries below begin as **NOT RUN** until an authorized tester
records the actual connection trace, external egress, DNS and failure behavior.
A failed chain must never fall back to unchained WARP.

## Experiment setup

- Fork: `ntun7729/usque-app`.
- Feature branch: `feature/android-wg-native-actor`.
- Android build workflow: `.github/workflows/usque-android-native.yml`.
- Preview application ID: `io.github.ntun7729.usque.experimental`.
- The preview APK uses a disposable build-only signing key. Back up existing
  profiles before uninstalling an incompatible earlier experimental APK.
- Generate/import a **WARP via WireGuard** configuration into Usque's encrypted
  chain library. Verify that it has a numeric Cloudflare endpoint IP, valid
  private/public keys, assigned addresses and permitted IP ranges. Do not put
  secrets in CI, screenshots, diagnostics, or bug reports.
- Under Advanced Settings → Transport → WARP connection, choose either
  MASQUE H3/H2 or Native WireGuard UDP. When using native UDP, select the
  imported WARP configuration in **Outer WARP WireGuard profile**. The final
  chain source is configured independently in Chain Proxy.
- Start with the same account, destination, test network and DNS settings
  across both modes. The native carrier needs UDP reachability to the chosen
  endpoint; blocked UDP must report failure without silently switching modes.
- Test with Android's VPN service enabled, plus SOCKS5 and HTTP-only
  frontends, and verify physical socket protection through Android logs.

## Live connection matrix

Each cell requires an actual handshake, a successful DNS lookup and a verified
remote egress IP. **No cell is marked PASS merely because an APK compiled.**

| Final chain exit | MASQUE H3/H2 | Native WireGuard UDP |
| --- | --- | --- |
| No chain / WARP only | NOT RUN | NOT RUN |
| OpenVPN custom (TCP and UDP) | NOT RUN | NOT RUN |
| Custom WireGuard | NOT RUN | NOT RUN |
| WARP via WireGuard chain | NOT RUN | NOT RUN |
| Proton VPN Guest | NOT RUN | NOT RUN |
| VPN Gate | NOT RUN | NOT RUN |
| HTTP CONNECT | NOT RUN | NOT RUN |
| SOCKS5 | NOT RUN | NOT RUN |

## Failure and regression matrix

| Scenario | Required behavior | Status |
| --- | --- | --- |
| Native UDP port blocked/unreachable | Timed failure, no MASQUE fallback or VPN leak | NOT RUN |
| Invalid/revoked outer WireGuard key | No connected state or unencrypted fallback | NOT RUN |
| Native profile missing from encrypted library | Explicit error, no connection | NOT RUN |
| Physical Wi-Fi ↔ mobile handover | Old socket generation rejected, new protected lease | NOT RUN |
| Android VpnService.protect refusal | Do not dial or forward through the TUN loop | NOT RUN |
| Kill switch + underlay loss | All protected application egress remains blocked | NOT RUN |
| Selected chain authentication rejected | No fallback to WARP-only egress | NOT RUN |
| Proton Guest expired credentials | Reauthorize or fail, without exposing user traffic | NOT RUN |
| IPv4/IPv6 DNS behavior | No unexpected physical DNS or IPv6 egress | NOT RUN |
| Split application routing/direct GEO rules | Only the explicit policy may bypass the VPN | NOT RUN |
| SOCKS5/HTTP frontend traffic | Same selected WARP/chain as Android TUN | NOT RUN |
| Repeat reconnect and cold mode switch | No duplicated TUN, leaked FDs or stale sessions | NOT RUN |
| Legacy profiles missing `warp_underlay` | MASQUE chosen; chain profile unchanged | CI-tested; device NOT RUN |

## What to record for each live test

Record Android version, app commit SHA, workflow run URL, network type,
requested WARP carrier, selected chain source, endpoint **without secrets**,
connection time, result of a DNS A/AAAA lookup, observed public egress
address/provider, approximate throughput and reconnect behavior. Record
**PASS**, **FAIL**, or **NOT RUN** independently for each scenario. Never
upload WireGuard private keys, Proton tokens, login sessions, or raw secrets.

## Release gate

Do not advertise dual transport as production-ready until the native actor
has a verified Android handshake, packet return path, DNS and leak-prevention
checks, the 2×7 exit matrix has been exercised, and the Android APK has been
verified for ABI, package ID, signing and bundled native libraries.
