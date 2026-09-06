//! USB device data model.
//!
//! Plain-data types shared by every backend that produces a snapshot of a
//! Linux USB tree. The actual sysfs walk lives in `usbeehive-sysfs`.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

/// A single interface descriptor of a USB device.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct UsbInterface {
    /// `bInterfaceNumber`.
    pub number: u32,
    /// `bInterfaceClass`.
    pub class_code: u8,
    /// `bInterfaceSubClass`.
    pub sub_class: u8,
    /// `bInterfaceProtocol`.
    pub protocol: u8,
    /// Driver name bound to this interface (`""` if none).
    pub driver: String,
}

/// Snapshot of a single USB device as observed in `/sys/bus/usb/devices/`.
///
/// Field naming mirrors common sysfs attribute names where possible. All
/// fields are public so the type is usable as a builder for tests and for
/// alternative backends.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct UsbDevice {
    /// Absolute sysfs path of the device directory.
    pub sysfs_path: PathBuf,
    /// Bus-port identifier as used in sysfs file names (e.g. `"1-1.4"`).
    pub bus_port: String,

    /// `idVendor`.
    pub vendor_id: u16,
    /// `idProduct`.
    pub product_id: u16,
    /// `manufacturer` string, or `""`.
    pub manufacturer: String,
    /// `product` string, or `""`.
    pub product: String,
    /// `serial` string, or `""`.
    pub serial: String,

    /// `version` string (e.g. `"2.10"`).
    pub version: String,
    /// Negotiated link speed in **Mbps**.
    pub speed: u32,
    /// `bMaxPower` × 2 (i.e. milliamps drawn from the upstream port).
    pub max_power_ma: u32,

    /// `bDeviceClass`.
    pub device_class: u8,
    /// `bDeviceSubClass`.
    pub device_sub_class: u8,
    /// `bDeviceProtocol`.
    pub device_protocol: u8,

    /// `busnum`.
    pub bus_num: u32,
    /// `devnum`.
    pub dev_num: u32,
    /// SuperSpeed RX-lane count.
    pub rx_lanes: u32,
    /// SuperSpeed TX-lane count.
    pub tx_lanes: u32,
    /// `removable` attribute (`"removable"`, `"fixed"`, `"unknown"`, `""`).
    pub removable: String,

    /// `bNumInterfaces`.
    pub num_interfaces: u32,
    /// `bNumConfigurations`.
    pub num_configurations: u32,

    /// Interfaces of the active configuration.
    pub interfaces: Vec<UsbInterface>,
    /// Direct children when this device is a hub.
    pub children: Vec<UsbDevice>,

    /// `device_class == 0x09`.
    pub is_hub: bool,
    /// Sysfs name starts with `"usb"` (a kernel-synthesized root hub).
    pub is_root_hub: bool,

    /// Decoded Binary Object Store (`bos_descriptors`), when the device
    /// publishes one.
    ///
    /// `None` is the normal case for USB 2.0-only devices — the kernel does
    /// not create the attribute at all. Where `speed` / `rx_lanes` /
    /// `tx_lanes` report what this link **negotiated**, this reports what the
    /// device is **capable of**; see [`crate::bos`].
    #[serde(default)]
    pub bos: Option<crate::bos::BosDescriptors>,

    /// Kernel per-device quirk bitmask (sysfs `quirks`, hex). Zero when the
    /// attribute is absent or no quirk applies. See [`USB_QUIRK_NO_BOS`].
    #[serde(default)]
    pub quirks: u32,

    /// Basename of the hub-port object this device hangs off — the target of
    /// the device's `port` symlink (e.g. `"usb5-port2"`, `"5-2.1-port1"`).
    /// Empty when the kernel publishes no `port` link (root hubs, or a
    /// backend that does not read it).
    #[serde(default)]
    pub port_id: String,

    /// Basename of the *companion* root-hub port that shares the same
    /// physical connector — the USB 2.0 half of a USB 3.x receptacle, or
    /// vice versa. Target of `<port>/peer`. Empty when there is none, which
    /// is the normal case for hub-internal ports.
    #[serde(default)]
    pub port_peer_id: String,

    /// The companion port's `state`, verbatim from the kernel
    /// (`"not attached"`, `"configured"`, `"suspended"`, `"powered"`, …).
    ///
    /// This is the *why* behind a capability shortfall: a SuperSpeed-capable
    /// device linked at 480 Mbps whose companion port reads `not attached`
    /// never trained its SuperSpeed lanes — a USB 2.0-only cable or
    /// receptacle. Empty when [`Self::port_peer_id`] is empty or the
    /// attribute is unreadable.
    #[serde(default)]
    pub port_peer_state: String,

    /// This device's own port `connect_type`, verbatim (`"hotplug"`,
    /// `"hardwired"`, `"not used"`). Empty when unreadable **or** when the
    /// kernel reports the literal `"unknown"`, which it does for every
    /// hub-internal port.
    #[serde(default)]
    pub port_connect_type: String,

    /// `maxchild` — number of downstream ports. Zero for non-hubs and when
    /// the attribute is unreadable.
    #[serde(default)]
    pub max_child: u32,

    /// Downstream ports of this hub whose `state` is `configured` or
    /// `suspended` — i.e. occupied. `None` when this is not a hub or no
    /// port objects were readable (which is not the same as "zero used").
    #[serde(default)]
    pub ports_used: Option<u32>,

    /// Active configuration descriptor `bmAttributes`. Bit 6 = self-powered,
    /// bit 5 = remote wakeup, bit 7 is reserved-and-always-set — so a value
    /// of zero means "not read", never a valid descriptor.
    #[serde(default)]
    pub bm_attributes: u32,

    /// Model name from the udev hardware database (`ID_MODEL_FROM_DATABASE`,
    /// which hwdb derives from the USB-IF `usb.ids` list). Empty when the
    /// lookup missed, when the crate was built without the `watch` feature
    /// (that is where the `udev` dependency lives), or when no udev database
    /// is present.
    ///
    /// Advisory only: it frequently names the *silicon* rather than the
    /// product (`"RTS5411 Hub"` for a device whose `iProduct` says
    /// `"4-Port USB 2.0 Hub"`), which is a capability clue, but it can also
    /// be stale for re-badged PIDs. Never overrides [`Self::product`].
    #[serde(default)]
    pub product_db: String,

    /// Every regular file in the device's sysfs directory, captured for
    /// `--raw` rendering. Optional: backends may leave this empty.
    pub raw_attributes: BTreeMap<String, String>,
}

/// `USB_QUIRK_NO_BOS` — `BIT(17)` in `include/linux/usb/quirks.h`.
///
/// When the kernel applies this quirk it never reads the device's Binary
/// Object Store, so `bos_descriptors` is absent **regardless of what the
/// device can actually do**. A missing BOS therefore means "unknown", not
/// "USB 2.0 only", whenever this bit is set.
pub const USB_QUIRK_NO_BOS: u32 = 1 << 17;

/// `USB_QUIRK_NO_LPM` — `BIT(10)` in `include/linux/usb/quirks.h`. Present
/// for completeness; it does not affect BOS visibility.
pub const USB_QUIRK_NO_LPM: u32 = 1 << 10;

/// `USB_QUIRK_*` bit names, index = bit position, snapshotted from
/// `include/linux/usb/quirks.h` of Linux 7.0 (bits 0–18; the `USB_QUIRK_`
/// prefix is stripped).
///
/// The kernel only ever appends here, so an older kernel is a prefix of this
/// table and a newer one grows past it — bits beyond the end are rendered as
/// `bit<N>` by [`quirk_names`] rather than dropped.
const QUIRK_BIT_NAMES: [&str; 19] = [
    "STRING_FETCH_255",
    "RESET_RESUME",
    "NO_SET_INTF",
    "CONFIG_INTF_STRINGS",
    "RESET",
    "HONOR_BNUMINTERFACES",
    "DELAY_INIT",
    "LINEAR_UFRAME_INTR_BINTERVAL",
    "DEVICE_QUALIFIER",
    "IGNORE_REMOTE_WAKEUP",
    "NO_LPM",
    "LINEAR_FRAME_INTR_BINTERVAL",
    "DISCONNECT_SUSPEND",
    "DELAY_CTRL_MSG",
    "HUB_SLOW_RESET",
    "ENDPOINT_IGNORE",
    "SHORT_SET_ADDRESS_REQ_TIMEOUT",
    "NO_BOS",
    "FORCE_ONE_CONFIG",
];

/// Decode a kernel quirk bitmask into names, in ascending bit order.
///
/// Bits this build does not know are rendered `bit<N>` so a newer kernel's
/// quirks are still reported rather than silently dropped. An empty result
/// means the kernel applied no workaround to the device.
///
/// ```
/// use usbeehive::usb::quirk_names;
///
/// assert!(quirk_names(0).is_empty());
/// assert_eq!(quirk_names(0x400), vec!["NO_LPM".to_string()]);
/// assert_eq!(quirk_names(1 << 31), vec!["bit31".to_string()]);
/// ```
pub fn quirk_names(quirks: u32) -> Vec<String> {
    (0..u32::BITS)
        .filter(|bit| quirks & (1 << bit) != 0)
        .map(|bit| match QUIRK_BIT_NAMES.get(bit as usize) {
            Some(name) => (*name).to_string(),
            None => format!("bit{bit}"),
        })
        .collect()
}

impl UsbDevice {
    /// Names of the kernel quirks applied to this device, ascending by bit.
    /// Empty when the kernel applied none — see [`quirk_names`].
    pub fn quirk_names(&self) -> Vec<String> {
        quirk_names(self.quirks)
    }

    /// `true` when the active configuration declares itself **self-powered**
    /// (`bmAttributes` bit 6), `false` when it declares itself bus-powered.
    ///
    /// `None` when `bmAttributes` was not read — bit 7 is reserved and
    /// always set in a valid descriptor, so zero can only mean "absent".
    pub fn self_powered(&self) -> Option<bool> {
        if self.bm_attributes == 0 {
            return None;
        }
        Some(self.bm_attributes & 0x40 != 0)
    }

    /// Current this hub can offer downstream, in mA, when it is bus-powered:
    /// 500 mA on a USB 2.0 link, 900 mA on a SuperSpeed one.
    ///
    /// `None` for non-hubs, for self-powered hubs (which draw from their own
    /// supply and have no bus-derived ceiling worth quoting), and when
    /// `bmAttributes` was not read.
    pub fn hub_power_budget_ma(&self) -> Option<u32> {
        if !self.is_hub || self.self_powered()? {
            return None;
        }
        Some(if self.speed >= 5000 { 900 } else { 500 })
    }

    /// Sum of the `bMaxPower` **declared** by this hub's direct children,
    /// counting self-powered children as zero (they draw from their own
    /// supply, not from this hub's budget).
    ///
    /// `None` when the hub has no enumerated children. This is a declared
    /// maximum, not a measured draw — never word it as "draws".
    pub fn hub_power_committed_ma(&self) -> Option<u32> {
        if self.children.is_empty() {
            return None;
        }
        Some(
            self.children
                .iter()
                .filter(|c| c.self_powered() != Some(true))
                .map(|c| c.max_power_ma)
                .sum(),
        )
    }
}

impl UsbDevice {
    /// `true` when the kernel is deliberately suppressing this device's BOS
    /// (`USB_QUIRK_NO_BOS`).
    ///
    /// Callers MUST NOT infer "USB 2.0 only" — or any capability shortfall —
    /// from a missing [`Self::bos`] while this is `true`. The device's
    /// capability is simply unknown.
    pub fn bos_suppressed_by_quirk(&self) -> bool {
        self.quirks & USB_QUIRK_NO_BOS != 0
    }

    /// Compare the negotiated link speed against the device's advertised
    /// capability, if it published a BOS.
    ///
    /// `None` when the device publishes no BOS — which is normal (USB
    /// 2.0-only devices have none) and must never be rendered as a fault.
    /// It is also `None`, for a different reason, when the kernel suppressed
    /// the BOS via `USB_QUIRK_NO_BOS`; use [`Self::bos_suppressed_by_quirk`]
    /// to tell the two apart. Either way no verdict is produced, so a
    /// quirked device can never be reported as degraded.
    pub fn data_rate(&self) -> Option<crate::bos::DataRateAssessment> {
        self.bos
            .as_ref()
            .map(|b| crate::bos::DataRateAssessment::evaluate(self.speed, b))
    }
}

/// USB link-speed tier negotiated for a device's upstream connection.
///
/// Bucketing matches the historical thresholds used by [`speed_label`]; the
/// `Mbps` value reported by sysfs is mapped to whichever variant covers it.
/// Two values report `Mbps == 0` for distinct reasons:
///
/// - The device negotiated a speed but sysfs returns it as a fractional
///   string (`"1.5"`) that fails int parse — those callers see [`Self::Low`]
///   if the raw value is `>= 2` and [`Self::Unknown`] otherwise.
/// - The kernel did not yet populate the attribute — those callers see
///   [`Self::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LinkSpeed {
    /// Speed not reported by sysfs.
    Unknown,
    /// USB 1.x Low Speed — 1.5 Mbps.
    Low,
    /// USB 1.x Full Speed — 12 Mbps.
    Full,
    /// USB 2.0 High Speed — 480 Mbps.
    High,
    /// USB 3.0 / 3.1 Gen 1 SuperSpeed — 5 Gbps.
    Super,
    /// USB 3.1 Gen 2 SuperSpeed+ — 10 Gbps.
    SuperPlus,
    /// USB 3.2 Gen 2x2 / USB4 — 20 Gbps.
    SuperPlus20,
    /// USB4 — 40 Gbps.
    Usb4,
}

impl LinkSpeed {
    /// Pretty-printed label, identical to the value previously returned by
    /// [`speed_label`].
    pub fn label(self) -> &'static str {
        match self {
            LinkSpeed::Usb4 => "USB4 40 Gbps",
            LinkSpeed::SuperPlus20 => "USB4 20 Gbps",
            LinkSpeed::SuperPlus => "SuperSpeed+ 10 Gbps",
            LinkSpeed::Super => "SuperSpeed 5 Gbps",
            LinkSpeed::High => "High Speed 480 Mbps",
            LinkSpeed::Full => "Full Speed 12 Mbps",
            LinkSpeed::Low => "Low Speed 1.5 Mbps",
            LinkSpeed::Unknown => "Unknown speed",
        }
    }
}

/// Bucket a Mbps figure into a [`LinkSpeed`] tier.
pub fn link_speed_tier(mbps: u32) -> LinkSpeed {
    match mbps {
        s if s >= 40000 => LinkSpeed::Usb4,
        s if s >= 20000 => LinkSpeed::SuperPlus20,
        s if s >= 10000 => LinkSpeed::SuperPlus,
        s if s >= 5000 => LinkSpeed::Super,
        s if s >= 480 => LinkSpeed::High,
        s if s >= 12 => LinkSpeed::Full,
        s if s >= 2 => LinkSpeed::Low,
        _ => LinkSpeed::Unknown,
    }
}

impl UsbDevice {
    /// Friendly name — `product` if present, else the `vid:pid` hex string.
    pub fn display_name(&self) -> String {
        if !self.product.is_empty() {
            self.product.clone()
        } else {
            format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
        }
    }

    /// Pretty-printed link speed (e.g. `"SuperSpeed+ 10 Gbps"`).
    pub fn speed_label(&self) -> &'static str {
        speed_label(self.speed)
    }

    /// Speed tier negotiated on this device's upstream link.
    pub fn link_speed_tier(&self) -> LinkSpeed {
        link_speed_tier(self.speed)
    }

    /// Power label (`"500 mA"` or `"1.5 W"`), or `None` if the device draws
    /// no recorded current.
    pub fn power_label(&self) -> Option<String> {
        if self.max_power_ma == 0 {
            return None;
        }
        Some(if self.max_power_ma >= 1000 {
            format!("{:.1} W", self.max_power_ma as f64 / 1000.0)
        } else {
            format!("{} mA", self.max_power_ma)
        })
    }

    /// Sysfs `bus_port` of this device's parent (`"5-2.4"` for `"5-2.4.1"`,
    /// `"usb1"` for `"1-1"`), or `None` if the device is a kernel root hub
    /// (no parent in the USB tree).
    ///
    /// The result is a sysfs identifier, not a path — pair with
    /// [`tree_roots`] when walking [`UsbDevice::children`].
    pub fn parent_bus_port(&self) -> Option<String> {
        if self.is_root_hub {
            return None;
        }
        if let Some((head, _)) = self.bus_port.rsplit_once('.') {
            return Some(head.to_string());
        }
        if let Some((bus, _)) = self.bus_port.split_once('-') {
            return Some(format!("usb{bus}"));
        }
        None
    }
}

/// Translate a Mbps figure to a USB speed-tier label. Equivalent to
/// `link_speed_tier(speed).label()`.
pub fn speed_label(speed: u32) -> &'static str {
    link_speed_tier(speed).label()
}

/// Devices in `devs` with no parent inside the same slice — kernel root hubs
/// (`is_root_hub == true`) plus orphans whose `parent_bus_port()` is not
/// enumerated. Use as the entry points for a topology walk over
/// [`UsbDevice::children`].
pub fn tree_roots(devs: &[UsbDevice]) -> Vec<&UsbDevice> {
    let names: HashSet<&str> = devs.iter().map(|d| d.bus_port.as_str()).collect();
    devs.iter()
        .filter(|d| match d.parent_bus_port() {
            None => true,
            Some(p) => !names.contains(p.as_str()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_label_thresholds() {
        assert_eq!(speed_label(0), "Unknown speed");
        assert_eq!(speed_label(2), "Low Speed 1.5 Mbps");
        assert_eq!(speed_label(12), "Full Speed 12 Mbps");
        assert_eq!(speed_label(480), "High Speed 480 Mbps");
        assert_eq!(speed_label(5000), "SuperSpeed 5 Gbps");
        assert_eq!(speed_label(10000), "SuperSpeed+ 10 Gbps");
        assert_eq!(speed_label(20000), "USB4 20 Gbps");
        assert_eq!(speed_label(40000), "USB4 40 Gbps");
    }

    #[test]
    fn link_speed_tier_buckets() {
        assert_eq!(link_speed_tier(0), LinkSpeed::Unknown);
        assert_eq!(link_speed_tier(2), LinkSpeed::Low);
        assert_eq!(link_speed_tier(12), LinkSpeed::Full);
        assert_eq!(link_speed_tier(480), LinkSpeed::High);
        assert_eq!(link_speed_tier(5_000), LinkSpeed::Super);
        assert_eq!(link_speed_tier(10_000), LinkSpeed::SuperPlus);
        assert_eq!(link_speed_tier(20_000), LinkSpeed::SuperPlus20);
        assert_eq!(link_speed_tier(40_000), LinkSpeed::Usb4);
    }

    #[test]
    fn link_speed_label_matches_legacy() {
        for mbps in [0, 2, 12, 480, 5_000, 10_000, 20_000, 40_000] {
            assert_eq!(link_speed_tier(mbps).label(), speed_label(mbps));
        }
    }

    #[test]
    fn parent_bus_port_resolves_levels() {
        let mk = |bp: &str, root: bool| UsbDevice {
            bus_port: bp.into(),
            is_root_hub: root,
            ..Default::default()
        };
        assert_eq!(
            mk("5-2.4.1", false).parent_bus_port().as_deref(),
            Some("5-2.4")
        );
        assert_eq!(mk("1-1", false).parent_bus_port().as_deref(), Some("usb1"));
        assert_eq!(mk("usb5", true).parent_bus_port(), None);
    }

    #[test]
    fn tree_roots_includes_root_hubs_and_orphans() {
        let root = UsbDevice {
            bus_port: "usb1".into(),
            is_root_hub: true,
            ..Default::default()
        };
        let attached = UsbDevice {
            bus_port: "1-1".into(),
            ..Default::default()
        };
        let orphan = UsbDevice {
            bus_port: "9-9".into(),
            ..Default::default()
        };
        let devs = vec![root, attached, orphan];
        let roots: Vec<&str> = tree_roots(&devs)
            .iter()
            .map(|d| d.bus_port.as_str())
            .collect();
        assert!(roots.contains(&"usb1"));
        assert!(roots.contains(&"9-9"));
        assert!(!roots.contains(&"1-1"));
    }

    #[test]
    fn power_label_formats() {
        let d = UsbDevice {
            max_power_ma: 0,
            ..Default::default()
        };
        assert!(d.power_label().is_none());
        let d = UsbDevice {
            max_power_ma: 100,
            ..Default::default()
        };
        assert_eq!(d.power_label().as_deref(), Some("100 mA"));
        let d = UsbDevice {
            max_power_ma: 1500,
            ..Default::default()
        };
        assert_eq!(d.power_label().as_deref(), Some("1.5 W"));
    }

    #[test]
    fn quirk_names_are_ascending_and_prefix_stripped() {
        assert!(quirk_names(0).is_empty());
        // 0x400 is the live value on this project's reference RTL8153.
        assert_eq!(quirk_names(USB_QUIRK_NO_LPM), vec!["NO_LPM"]);
        assert_eq!(quirk_names(USB_QUIRK_NO_BOS), vec!["NO_BOS"]);
        assert_eq!(
            quirk_names(USB_QUIRK_NO_BOS | (1 << 1) | USB_QUIRK_NO_LPM),
            vec!["RESET_RESUME", "NO_LPM", "NO_BOS"]
        );
    }

    #[test]
    fn quirk_names_render_unknown_bits_rather_than_dropping_them() {
        // Bit 19 does not exist in the 7.0 header snapshot; a newer kernel
        // must still be reported, not silently swallowed.
        assert_eq!(quirk_names(1 << 19), vec!["bit19"]);
        assert_eq!(quirk_names(1 << 31), vec!["bit31"]);
        assert_eq!(
            quirk_names(USB_QUIRK_NO_BOS | (1 << 19)),
            ["NO_BOS", "bit19"]
        );
    }

    #[test]
    fn self_powered_reads_bmattributes_bit6() {
        let mk = |bm: u32| UsbDevice {
            bm_attributes: bm,
            ..Default::default()
        };
        // Live values from the reference machine.
        assert_eq!(mk(0xe0).self_powered(), Some(true)); // 3-6, 5-2
        assert_eq!(mk(0xc0).self_powered(), Some(true)); // 5-2.1.2
        assert_eq!(mk(0xa0).self_powered(), Some(false)); // 5-2.4
        assert_eq!(mk(0x80).self_powered(), Some(false)); // 3-5
        assert_eq!(mk(0).self_powered(), None); // attribute absent
    }

    #[test]
    fn hub_power_budget_only_for_bus_powered_hubs() {
        let mk = |is_hub: bool, bm: u32, speed: u32| UsbDevice {
            is_hub,
            bm_attributes: bm,
            speed,
            ..Default::default()
        };
        assert_eq!(mk(true, 0xa0, 12).hub_power_budget_ma(), Some(500));
        assert_eq!(mk(true, 0xa0, 5000).hub_power_budget_ma(), Some(900));
        // Self-powered hub: no bus-derived ceiling worth quoting.
        assert_eq!(mk(true, 0xe0, 480).hub_power_budget_ma(), None);
        // Not a hub, and unread bmAttributes.
        assert_eq!(mk(false, 0xa0, 480).hub_power_budget_ma(), None);
        assert_eq!(mk(true, 0, 480).hub_power_budget_ma(), None);
    }

    #[test]
    fn hub_power_committed_sums_bus_powered_children_only() {
        let child = |ma: u32, bm: u32| UsbDevice {
            max_power_ma: ma,
            bm_attributes: bm,
            ..Default::default()
        };
        let mut hub = UsbDevice {
            is_hub: true,
            bm_attributes: 0xa0,
            ..Default::default()
        };
        assert_eq!(hub.hub_power_committed_ma(), None);

        // Reference machine's 5-2.4: keyboard 90 mA + mouse 98 mA.
        hub.children = vec![child(90, 0xa0), child(98, 0xa0)];
        assert_eq!(hub.hub_power_committed_ma(), Some(188));

        // A self-powered child draws from its own supply, not this budget.
        hub.children.push(child(500, 0xe0));
        assert_eq!(hub.hub_power_committed_ma(), Some(188));

        // A child whose bmAttributes could not be read is counted — the
        // conservative direction for a budget.
        hub.children.push(child(100, 0));
        assert_eq!(hub.hub_power_committed_ma(), Some(288));
    }

    #[test]
    fn display_name_falls_back_to_vidpid() {
        let mut d = UsbDevice {
            vendor_id: 0x05AC,
            product_id: 0x12A8,
            ..Default::default()
        };
        assert_eq!(d.display_name(), "05ac:12a8");
        d.product = "iPhone".into();
        assert_eq!(d.display_name(), "iPhone");
    }
}
