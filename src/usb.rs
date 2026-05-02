//! USB device enumeration from `/sys/bus/usb/devices/`.
//!
//! sysfs interleaves device-level entries (`1-1`, `usb1`) with interface
//! entries (`1-1:1.0`) in the same directory. We treat any name containing
//! a colon as an interface child of its parent device.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::sysfs;

const USB_BASE: &str = "/sys/bus/usb/devices";

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsbInterface {
    pub number: u32,
    pub class_code: u8,
    pub sub_class: u8,
    pub protocol: u8,
    pub driver: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsbDevice {
    pub sysfs_path: PathBuf,
    pub bus_port: String,

    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,

    pub version: String,
    /// Negotiated link speed in Mbps.
    pub speed: u32,
    pub max_power_ma: u32,

    pub device_class: u8,
    pub device_sub_class: u8,
    pub device_protocol: u8,

    pub bus_num: u32,
    pub dev_num: u32,
    pub rx_lanes: u32,
    pub tx_lanes: u32,
    pub removable: String,

    pub num_interfaces: u32,
    pub num_configurations: u32,

    pub interfaces: Vec<UsbInterface>,
    pub children: Vec<UsbDevice>,

    pub is_hub: bool,
    pub is_root_hub: bool,

    pub raw_attributes: std::collections::BTreeMap<String, String>,
}

impl UsbDevice {
    pub fn display_name(&self) -> String {
        if !self.product.is_empty() {
            self.product.clone()
        } else {
            format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
        }
    }

    pub fn speed_label(&self) -> &'static str {
        speed_label(self.speed)
    }

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

    pub fn enumerate() -> Vec<UsbDevice> {
        enumerate_in(Path::new(USB_BASE))
    }
}

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

/// `bMaxPower` may be reported as `"500mA"` or just `"500"`. We pull the
/// leading run of digits.
fn parse_max_power(s: &str) -> u32 {
    let mut n: u32 = 0;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add((b - b'0') as u32);
        } else if n > 0 {
            break;
        }
    }
    n
}

fn read_interfaces(dev_path: &Path) -> Vec<UsbInterface> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dev_path) else {
        return out;
    };
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.contains(':') {
            continue;
        }
        let if_path = entry.path();
        let Some(class) = sysfs::read_hex(if_path.join("bInterfaceClass")) else {
            continue;
        };
        let driver = fs::read_link(if_path.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let number = name
            .rsplit_once('.')
            .and_then(|(_, n)| n.parse().ok())
            .unwrap_or(0);
        out.push(UsbInterface {
            number,
            class_code: class as u8,
            sub_class: sysfs::read_hex(if_path.join("bInterfaceSubClass")).unwrap_or(0) as u8,
            protocol: sysfs::read_hex(if_path.join("bInterfaceProtocol")).unwrap_or(0) as u8,
            driver,
        });
    }
    out
}

fn from_sysfs(path: &Path, name: &str) -> Option<UsbDevice> {
    if name.contains(':') {
        return None;
    }
    let vid = sysfs::read_hex(path.join("idVendor"))?;
    let pid = sysfs::read_hex(path.join("idProduct"))?;

    let device_class = sysfs::read_hex(path.join("bDeviceClass")).unwrap_or(0) as u8;

    let dev = UsbDevice {
        sysfs_path: path.to_path_buf(),
        bus_port: name.to_string(),
        vendor_id: vid as u16,
        product_id: pid as u16,
        manufacturer: sysfs::read_attr(path.join("manufacturer")).unwrap_or_default(),
        product: sysfs::read_attr(path.join("product")).unwrap_or_default(),
        serial: sysfs::read_attr(path.join("serial")).unwrap_or_default(),
        version: sysfs::read_attr(path.join("version")).unwrap_or_default(),
        removable: sysfs::read_attr(path.join("removable")).unwrap_or_default(),

        speed: sysfs::read_int(path.join("speed")).unwrap_or(0).max(0) as u32,
        max_power_ma: parse_max_power(
            &sysfs::read_attr(path.join("bMaxPower")).unwrap_or_default(),
        ),
        bus_num: sysfs::read_int(path.join("busnum")).unwrap_or(0).max(0) as u32,
        dev_num: sysfs::read_int(path.join("devnum")).unwrap_or(0).max(0) as u32,
        rx_lanes: sysfs::read_int(path.join("rx_lanes")).unwrap_or(0).max(0) as u32,
        tx_lanes: sysfs::read_int(path.join("tx_lanes")).unwrap_or(0).max(0) as u32,
        num_configurations: sysfs::read_int(path.join("bNumConfigurations"))
            .unwrap_or(0)
            .max(0) as u32,
        num_interfaces: sysfs::read_attr(path.join("bNumInterfaces"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),

        device_class,
        device_sub_class: sysfs::read_hex(path.join("bDeviceSubClass")).unwrap_or(0) as u8,
        device_protocol: sysfs::read_hex(path.join("bDeviceProtocol")).unwrap_or(0) as u8,

        is_hub: device_class == 0x09,
        is_root_hub: name.starts_with("usb"),

        interfaces: read_interfaces(path),
        children: Vec::new(),
        raw_attributes: sysfs::read_all_attrs(path),
        ..Default::default()
    };

    Some(dev)
}

/// Attach each non-root device as a child of its parent (by `bus_port` name).
fn build_topology(devices: &mut [UsbDevice]) {
    let mut idx: HashMap<String, usize> = HashMap::with_capacity(devices.len());
    for (i, d) in devices.iter().enumerate() {
        idx.insert(d.bus_port.clone(), i);
    }
    let snapshots: Vec<UsbDevice> = devices.to_vec();
    for d in &snapshots {
        if d.is_root_hub {
            continue;
        }
        let parent = parent_bus_port(&d.bus_port);
        if let Some(&pi) = idx.get(&parent) {
            devices[pi].children.push(d.clone());
        }
    }
}

fn parent_bus_port(bp: &str) -> String {
    if let Some((head, _)) = bp.rsplit_once('.') {
        return head.to_string();
    }
    if let Some((bus, _)) = bp.split_once('-') {
        return format!("usb{bus}");
    }
    String::new()
}

fn enumerate_in(base: &Path) -> Vec<UsbDevice> {
    let mut entries: Vec<(PathBuf, String)> = sysfs::subdirs(base)
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            Some((p, name))
        })
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    let mut devices: Vec<UsbDevice> = entries
        .iter()
        .filter_map(|(p, n)| from_sysfs(p, n))
        .collect();
    build_topology(&mut devices);
    devices
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
    fn parse_max_power_strips_units() {
        assert_eq!(parse_max_power("500mA"), 500);
        assert_eq!(parse_max_power("100"), 100);
        assert_eq!(parse_max_power(""), 0);
        assert_eq!(parse_max_power("garbage"), 0);
    }

    #[test]
    fn parent_bus_port_rules() {
        assert_eq!(parent_bus_port("1-1.4.2"), "1-1.4");
        assert_eq!(parent_bus_port("1-1"), "usb1");
        assert_eq!(parent_bus_port("usb1"), "");
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
