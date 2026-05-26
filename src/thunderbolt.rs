//! Thunderbolt / USB4 router data model.
//!
//! Plain-data type corresponding to entries under
//! `/sys/bus/thunderbolt/devices/`. Enumeration lives in `usbeehive-sysfs`.
//!
//! USB4 cannot be detected as a Type-C alternate mode — it's negotiated at
//! the USB-PD enter-mode layer and surfaces through the `thunderbolt`
//! subsystem instead. A router with `generation == 4` is the kernel's
//! signal that the link speaks the USB4 protocol (TBT4 is USB4 with Intel
//! branding); `generation == 3` is TBT3-only.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

/// One entry under `/sys/bus/thunderbolt/devices/`.
///
/// The kernel publishes every router on every Thunderbolt / USB4 domain
/// here. Host adapters carry a `-0` route suffix (`0-0`, `1-0`, …) and
/// represent the local controller; non-host routers are devices reachable
/// through the host (`0-1`, `0-1-1`, …).
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ThunderboltRouter {
    /// Absolute sysfs path of the router directory.
    pub sysfs_path: PathBuf,
    /// Kernel-assigned route string (e.g. `"0-0"`, `"0-1"`, `"0-1-1"`).
    pub route: String,
    /// Numeric domain index parsed from the route prefix.
    pub domain: u32,
    /// `true` when this router is a host adapter (route ends with `-0`).
    pub is_host: bool,
    /// Router generation. `1` = original Thunderbolt, `2` = TBT2, `3` = TBT3,
    /// `4` = USB4 (Intel markets this as Thunderbolt 4). `0` when unreadable.
    pub generation: u8,
    /// Vendor ID from the `vendor` attribute.
    pub vendor_id: u16,
    /// Device ID from the `device` attribute.
    pub device_id: u16,
    /// `vendor_name` attribute.
    pub vendor_name: String,
    /// `device_name` attribute.
    pub device_name: String,
    /// `unique_id` attribute — stable across reboots.
    pub unique_id: String,
    /// All regular files under the router directory.
    pub raw_attributes: BTreeMap<String, String>,
}

impl ThunderboltRouter {
    /// `true` when this router negotiated the USB4 protocol
    /// (i.e. `generation >= 4`). TBT3-only routers return `false`.
    pub fn supports_usb4(&self) -> bool {
        self.generation >= 4
    }
}

/// Sentinel for "the local system has at least one USB4-capable host
/// adapter AND at least one non-host router has negotiated USB4."
///
/// USB4 is link-local, but the kernel's `thunderbolt` sysfs doesn't link
/// individual routers back to specific Type-C ports — so this is the
/// strongest "USB4 link in use somewhere on this system" signal available
/// without a separate port-mapping table.
pub fn system_has_usb4_link(routers: &[ThunderboltRouter]) -> bool {
    let host_supports = routers.iter().any(|r| r.is_host && r.supports_usb4());
    if !host_supports {
        return false;
    }
    routers.iter().any(|r| !r.is_host && r.supports_usb4())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router(route: &str, is_host: bool, generation: u8) -> ThunderboltRouter {
        ThunderboltRouter {
            route: route.into(),
            is_host,
            generation,
            ..Default::default()
        }
    }

    #[test]
    fn supports_usb4_requires_generation_4() {
        assert!(router("0-0", true, 4).supports_usb4());
        assert!(!router("0-0", true, 3).supports_usb4());
        assert!(!router("0-0", true, 0).supports_usb4());
    }

    #[test]
    fn system_has_usb4_link_needs_both_sides() {
        // Just a USB4 host, no devices → no active link.
        assert!(!system_has_usb4_link(&[router("0-0", true, 4)]));
        // USB4 device but a TBT3-only host → no USB4 link (host can't drive it).
        assert!(!system_has_usb4_link(&[
            router("0-0", true, 3),
            router("0-1", false, 4),
        ]));
        // Both sides USB4 → link active.
        assert!(system_has_usb4_link(&[
            router("0-0", true, 4),
            router("0-1", false, 4),
        ]));
    }

    #[test]
    fn system_has_usb4_link_ignores_tbt3_partner_under_usb4_host() {
        // Backwards-compat dock — TBT3 partner on a USB4 host. Link runs at
        // TBT3, not USB4.
        assert!(!system_has_usb4_link(&[
            router("0-0", true, 4),
            router("0-1", false, 3),
        ]));
    }
}
