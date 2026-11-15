//! Selection of the outer WARP transport, independent of the chain exit.
//!
//! This contract does not enable native WireGuard by itself: the selected
//! transport must be supported by the active runtime before connecting.

use serde::{Deserialize, Serialize};

/// The WARP transport carrying application and optional chain-exit packets.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WarpUnderlay {
    /// Existing QUIC/HTTP CONNECT-IP path; preserves legacy behavior.
    #[default]
    Masque,
    /// Direct UDP WireGuard path, without a MASQUE outer connection.
    #[serde(rename = "native_wireguard")]
    NativeWireGuard,
}

impl WarpUnderlay {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Masque => "masque",
            Self::NativeWireGuard => "native_wireguard",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WarpUnderlay;
    use serde::Deserialize;

    #[test]
    fn warp_underlay_serializes_with_stable_names() {
        for mode in [WarpUnderlay::Masque, WarpUnderlay::NativeWireGuard] {
            let encoded = serde_json::to_string(&mode).unwrap();
            assert_eq!(encoded, format!("\"{}\"", mode.as_str()));
            assert_eq!(serde_json::from_str::<WarpUnderlay>(&encoded).unwrap(), mode);
        }
    }

    #[test]
    fn warp_underlay_rejects_unknown_modes() {
        for invalid in ["\"wireguard\"", "\"native_wire_guard\"", "\"auto\"", "\"l4_proxy\"", "null", "3"] {
            assert!(serde_json::from_str::<WarpUnderlay>(invalid).is_err());
        }
    }

    #[test]
    fn warp_underlay_defaults_to_masque_for_existing_profiles() {
        #[derive(Deserialize)]
        struct ExistingProfile {
            #[serde(default)]
            warp_underlay: WarpUnderlay,
        }

        let profile: ExistingProfile = serde_json::from_str("{}").unwrap();
        assert_eq!(profile.warp_underlay, WarpUnderlay::Masque);
        assert_eq!(WarpUnderlay::default(), WarpUnderlay::Masque);
    }
}
