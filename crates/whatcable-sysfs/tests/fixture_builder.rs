//! Programmatic builder for hand-crafted sysfs trees used by integration tests.
//!
//! sysfs is a directory of small text files; replicating that on a tmpdir
//! gives us deterministic, portable fixtures without needing data captured
//! from a specific machine.

#![allow(dead_code, missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};

/// A scratch directory cleaned up on `Drop`. Spelled out by hand to avoid a
/// `tempfile` dev-dependency.
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("wcfix-{label}-{pid}-{n}"));
        fs::create_dir_all(&path).unwrap();
        TempRoot { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn write_attr(dir: &Path, name: &str, value: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), value).unwrap();
}

/// Add a minimal USB device entry under `<root>/bus/usb/devices/<bus_port>/`.
pub struct UsbDeviceFixture<'a> {
    pub bus_port: &'a str,
    pub vendor: u16,
    pub product: u16,
    pub product_name: &'a str,
    pub manufacturer: &'a str,
    pub serial: &'a str,
    pub speed_mbps: u32,
    pub max_power_ma: u32,
    pub version: &'a str,
    pub device_class: u8,
    pub bus_num: u32,
    pub dev_num: u32,
    pub interfaces: &'a [InterfaceFixture<'a>],
    pub removable: &'a str,
}

pub struct InterfaceFixture<'a> {
    pub number: u32,
    pub class: u8,
    pub sub_class: u8,
    pub protocol: u8,
    pub driver: &'a str,
}

impl<'a> UsbDeviceFixture<'a> {
    pub fn write(&self, root: &Path) {
        let usb_dir = root.join("bus/usb/devices").join(self.bus_port);
        fs::create_dir_all(&usb_dir).unwrap();
        write_attr(&usb_dir, "idVendor", &format!("{:04x}", self.vendor));
        write_attr(&usb_dir, "idProduct", &format!("{:04x}", self.product));
        if !self.product_name.is_empty() {
            write_attr(&usb_dir, "product", self.product_name);
        }
        if !self.manufacturer.is_empty() {
            write_attr(&usb_dir, "manufacturer", self.manufacturer);
        }
        if !self.serial.is_empty() {
            write_attr(&usb_dir, "serial", self.serial);
        }
        write_attr(&usb_dir, "speed", &self.speed_mbps.to_string());
        write_attr(&usb_dir, "bMaxPower", &format!("{}mA", self.max_power_ma));
        write_attr(&usb_dir, "version", self.version);
        write_attr(&usb_dir, "bDeviceClass", &format!("{:02x}", self.device_class));
        write_attr(&usb_dir, "busnum", &self.bus_num.to_string());
        write_attr(&usb_dir, "devnum", &self.dev_num.to_string());
        write_attr(&usb_dir, "removable", self.removable);
        write_attr(
            &usb_dir,
            "bNumInterfaces",
            &self.interfaces.len().to_string(),
        );
        for iface in self.interfaces {
            let if_dir = usb_dir.join(format!("{}:1.{}", self.bus_port, iface.number));
            fs::create_dir_all(&if_dir).unwrap();
            write_attr(&if_dir, "bInterfaceClass", &format!("{:02x}", iface.class));
            write_attr(&if_dir, "bInterfaceSubClass", &format!("{:02x}", iface.sub_class));
            write_attr(&if_dir, "bInterfaceProtocol", &format!("{:02x}", iface.protocol));
            if !iface.driver.is_empty() {
                let driver_target = root.join(format!("bus/usb/drivers/{}", iface.driver));
                fs::create_dir_all(&driver_target).unwrap();
                std::os::unix::fs::symlink(&driver_target, if_dir.join("driver")).unwrap();
            }
        }
    }
}

/// Helper: write a Type-C port directory (without partner/cable).
pub fn write_typec_port(root: &Path, port_name: &str, fields: &[(&str, &str)]) -> PathBuf {
    let dir = root.join("class/typec").join(port_name);
    fs::create_dir_all(&dir).unwrap();
    for (k, v) in fields {
        write_attr(&dir, k, v);
    }
    dir
}

/// Helper: write a Type-C cable directory `<port>-cable` with VDOs.
pub fn write_typec_cable(
    root: &Path,
    port_name: &str,
    cable_type: &str,
    plug_type: &str,
    vdos: &[(&str, u32)],
) {
    let dir = root.join("class/typec").join(format!("{port_name}-cable"));
    fs::create_dir_all(&dir).unwrap();
    write_attr(&dir, "type", cable_type);
    write_attr(&dir, "plug_type", plug_type);
    let id = dir.join("identity");
    fs::create_dir_all(&id).unwrap();
    for (name, raw) in vdos {
        write_attr(&id, name, &format!("0x{raw:08x}"));
    }
}

/// Helper: write a Type-C partner directory with optional VDOs.
pub fn write_typec_partner(
    root: &Path,
    port_name: &str,
    partner_type: &str,
    vdos: &[(&str, u32)],
) {
    let dir = root.join("class/typec").join(format!("{port_name}-partner"));
    fs::create_dir_all(&dir).unwrap();
    write_attr(&dir, "type", partner_type);
    if !vdos.is_empty() {
        let id = dir.join("identity");
        fs::create_dir_all(&id).unwrap();
        for (name, raw) in vdos {
            write_attr(&id, name, &format!("0x{raw:08x}"));
        }
    }
}

/// Helper: write a USB-PD port with `source-capabilities` PDOs.
pub struct PdoFixture {
    pub voltage_mv: u32,
    pub current_ma: u32,
    pub power_mw: u32,
    pub kind: &'static str, // "fixed_supply", "battery", "variable_supply", "programmable_supply (pps)"
    pub min_voltage_mv: u32,
    pub max_voltage_mv: u32,
}

pub fn write_pd_port(
    root: &Path,
    port_name: &str,
    parent_port: i32,
    source_pdos: &[PdoFixture],
) {
    let dir = root.join("class/usb_power_delivery").join(port_name);
    let caps = dir.join("source-capabilities");
    fs::create_dir_all(&caps).unwrap();
    if parent_port >= 0 {
        write_attr(&dir, "parent_port_number", &parent_port.to_string());
    }
    for (i, p) in source_pdos.iter().enumerate() {
        let entry = caps.join(format!("{}:fixed_supply", i + 1));
        fs::create_dir_all(&entry).unwrap();
        write_attr(&entry, "type", p.kind);
        if p.voltage_mv > 0 {
            write_attr(&entry, "voltage", &p.voltage_mv.to_string());
        }
        if p.min_voltage_mv > 0 {
            write_attr(&entry, "minimum_voltage", &p.min_voltage_mv.to_string());
        }
        if p.max_voltage_mv > 0 {
            write_attr(&entry, "maximum_voltage", &p.max_voltage_mv.to_string());
        }
        if p.current_ma > 0 {
            write_attr(&entry, "maximum_current", &p.current_ma.to_string());
        }
        if p.power_mw > 0 {
            write_attr(&entry, "maximum_power", &p.power_mw.to_string());
        }
    }
}
