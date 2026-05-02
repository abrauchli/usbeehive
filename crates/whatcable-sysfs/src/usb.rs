//! USB device enumeration from `/sys/bus/usb/devices/`.
//!
//! sysfs interleaves device-level entries (`1-1`, `usb1`) with interface
//! entries (`1-1:1.0`) in the same directory. We treat any name containing
//! a colon as an interface child of its parent device.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use whatcable_core::usb::{UsbDevice, UsbInterface};

use crate::sysfs::{self, Sysfs};

impl Sysfs {
    /// Walk this sysfs root's USB devices directory and return a snapshot.
    ///
    /// Returns an empty `Vec` if the directory is missing.
    pub fn usb_devices(&self) -> Vec<UsbDevice> {
        enumerate_in(&self.usb_devices_dir())
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
    out.sort_by_key(|i| i.number);
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
    };

    Some(dev)
}

/// Attach each non-root device as a child of its parent (by `bus_port` name).
///
/// Processed deepest-first so a child's `children` vector is fully populated
/// before the child itself is cloned into its parent.
fn build_topology(devices: &mut [UsbDevice]) {
    let mut idx: HashMap<String, usize> = HashMap::with_capacity(devices.len());
    for (i, d) in devices.iter().enumerate() {
        idx.insert(d.bus_port.clone(), i);
    }

    let mut order: Vec<usize> = (0..devices.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(bus_port_depth(&devices[i].bus_port)));

    for i in order {
        if devices[i].is_root_hub {
            continue;
        }
        let parent = parent_bus_port(&devices[i].bus_port);
        if let Some(&pi) = idx.get(&parent) {
            if pi == i {
                continue;
            }
            let child = devices[i].clone();
            devices[pi].children.push(child);
        }
    }
}

fn bus_port_depth(bp: &str) -> usize {
    if bp.starts_with("usb") {
        0
    } else {
        bp.matches('.').count() + 1
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

pub(crate) fn enumerate_in(base: &Path) -> Vec<UsbDevice> {
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
    fn bus_port_depth_counts_dots() {
        assert_eq!(bus_port_depth("usb1"), 0);
        assert_eq!(bus_port_depth("1-1"), 1);
        assert_eq!(bus_port_depth("1-1.2"), 2);
        assert_eq!(bus_port_depth("1-1.2.3"), 3);
    }

    #[test]
    fn enumerate_missing_dir_returns_empty() {
        let result = enumerate_in(Path::new("/no/such/whatcable/path"));
        assert!(result.is_empty());
    }
}
