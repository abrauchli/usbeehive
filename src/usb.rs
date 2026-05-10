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

    /// Every regular file in the device's sysfs directory, captured for
    /// `--raw` rendering. Optional: backends may leave this empty.
    pub raw_attributes: BTreeMap<String, String>,
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
        assert_eq!(mk("5-2.4.1", false).parent_bus_port().as_deref(), Some("5-2.4"));
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
        let roots: Vec<&str> = tree_roots(&devs).iter().map(|d| d.bus_port.as_str()).collect();
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
