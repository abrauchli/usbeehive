//! USB device data model.
//!
//! Plain-data types shared by every backend that produces a snapshot of a
//! Linux USB tree. The actual sysfs walk lives in `whatcable-sysfs`.

use std::collections::BTreeMap;
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
}

/// Translate a Mbps figure to a USB speed-tier label.
pub fn speed_label(speed: u32) -> &'static str {
    match speed {
        s if s >= 20000 => "USB4 20 Gbps",
        s if s >= 10000 => "SuperSpeed+ 10 Gbps",
        s if s >= 5000 => "SuperSpeed 5 Gbps",
        s if s >= 480 => "High Speed 480 Mbps",
        s if s >= 12 => "Full Speed 12 Mbps",
        s if s >= 2 => "Low Speed 1.5 Mbps",
        _ => "Unknown speed",
    }
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
    }

    #[test]
    fn power_label_formats() {
        let mut d = UsbDevice::default();
        d.max_power_ma = 0;
        assert!(d.power_label().is_none());
        d.max_power_ma = 100;
        assert_eq!(d.power_label().as_deref(), Some("100 mA"));
        d.max_power_ma = 1500;
        assert_eq!(d.power_label().as_deref(), Some("1.5 W"));
    }

    #[test]
    fn display_name_falls_back_to_vidpid() {
        let mut d = UsbDevice::default();
        d.vendor_id = 0x05AC;
        d.product_id = 0x12A8;
        assert_eq!(d.display_name(), "05ac:12a8");
        d.product = "iPhone".into();
        assert_eq!(d.display_name(), "iPhone");
    }
}
