#![cfg(feature = "sysfs")]

//! Integration tests: hand-crafted sysfs trees → DeviceManager snapshots.

mod fixture_builder;
use fixture_builder::*;

use whatcable::{DeviceManager, Sysfs};

#[test]
fn enumerates_root_hub_with_keyboard_and_storage() {
    let root = TempRoot::new("usb-basic");

    UsbDeviceFixture {
        bus_port: "usb1",
        vendor: 0x1D6B,
        product: 0x0002,
        product_name: "xHCI Host Controller",
        manufacturer: "Linux 6.17",
        serial: "0000:01:00.0",
        speed_mbps: 480,
        max_power_ma: 0,
        version: "2.00",
        device_class: 0x09,
        bus_num: 1,
        dev_num: 1,
        removable: "unknown",
        interfaces: &[InterfaceFixture {
            number: 0,
            class: 0x09,
            sub_class: 0,
            protocol: 0,
            driver: "hub",
        }],
    }
    .write(root.path());

    UsbDeviceFixture {
        bus_port: "1-1",
        vendor: 0x046D,
        product: 0xC31C,
        product_name: "USB Keyboard",
        manufacturer: "Logitech",
        serial: "",
        speed_mbps: 12,
        max_power_ma: 100,
        version: "2.00",
        device_class: 0x00,
        bus_num: 1,
        dev_num: 2,
        removable: "removable",
        interfaces: &[InterfaceFixture {
            number: 0,
            class: 0x03,
            sub_class: 1,
            protocol: 1,
            driver: "usbhid",
        }],
    }
    .write(root.path());

    UsbDeviceFixture {
        bus_port: "1-2",
        vendor: 0x0951,
        product: 0x1666,
        product_name: "DataTraveler 3.0",
        manufacturer: "Kingston",
        serial: "ABCD1234",
        speed_mbps: 5000,
        max_power_ma: 896,
        version: "3.20",
        device_class: 0x00,
        bus_num: 1,
        dev_num: 3,
        removable: "removable",
        interfaces: &[InterfaceFixture {
            number: 0,
            class: 0x08,
            sub_class: 6,
            protocol: 0x50,
            driver: "usb-storage",
        }],
    }
    .write(root.path());

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let snap = mgr.snapshot();
    assert_eq!(snap.usb_devices.len(), 3, "expected 3 devices");

    // Topology: usb1 should have two children
    let root_hub = snap
        .usb_devices
        .iter()
        .find(|d| d.bus_port == "usb1")
        .unwrap();
    assert_eq!(root_hub.children.len(), 2);
    assert!(root_hub.is_root_hub);

    // Summaries omit root hubs, leaving the keyboard + storage
    assert_eq!(snap.summaries.len(), 2);

    let kb = snap
        .summaries
        .iter()
        .find(|s| s.headline == "USB Keyboard")
        .unwrap();
    assert!(kb.subtitle.contains("Logitech"));
    assert!(kb.subtitle.contains("HID"));
    assert!(kb.bullets.iter().any(|b| b == "Drivers: usbhid"));

    let storage = snap
        .summaries
        .iter()
        .find(|s| s.headline == "DataTraveler 3.0")
        .unwrap();
    assert!(storage.subtitle.contains("Kingston"));
    assert!(storage.subtitle.contains("Mass Storage"));
    assert!(storage.bullets.iter().any(|b| b == "Serial: ABCD1234"));
}

#[test]
fn missing_attrs_yield_skip_not_panic() {
    let root = TempRoot::new("usb-missing");
    // A device dir with no idVendor / idProduct should be silently skipped.
    let dir = root.path().join("bus/usb/devices/1-3");
    std::fs::create_dir_all(&dir).unwrap();

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();
    assert!(mgr.snapshot().usb_devices.is_empty());
}

#[test]
fn deep_topology_preserves_parent_chain() {
    let root = TempRoot::new("usb-tree");

    UsbDeviceFixture {
        bus_port: "usb5",
        vendor: 0x1D6B,
        product: 0x0003,
        product_name: "xHCI Host Controller",
        manufacturer: "",
        serial: "",
        speed_mbps: 5000,
        max_power_ma: 0,
        version: "3.00",
        device_class: 0x09,
        bus_num: 5,
        dev_num: 1,
        removable: "unknown",
        interfaces: &[],
    }
    .write(root.path());

    // 5-2 = top-level hub
    UsbDeviceFixture {
        bus_port: "5-2",
        vendor: 0x05E3,
        product: 0x0610,
        product_name: "USB 2.0 Hub",
        manufacturer: "Genesys Logic",
        serial: "",
        speed_mbps: 480,
        max_power_ma: 100,
        version: "2.00",
        device_class: 0x09,
        bus_num: 5,
        dev_num: 2,
        removable: "removable",
        interfaces: &[],
    }
    .write(root.path());

    // 5-2.1 = nested hub
    UsbDeviceFixture {
        bus_port: "5-2.1",
        vendor: 0x05E3,
        product: 0x0610,
        product_name: "USB 2.0 Hub",
        manufacturer: "Genesys Logic",
        serial: "",
        speed_mbps: 480,
        max_power_ma: 100,
        version: "2.00",
        device_class: 0x09,
        bus_num: 5,
        dev_num: 3,
        removable: "removable",
        interfaces: &[],
    }
    .write(root.path());

    // 5-2.1.1 = leaf device (mouse)
    UsbDeviceFixture {
        bus_port: "5-2.1.1",
        vendor: 0x046D,
        product: 0xC52B,
        product_name: "USB Receiver",
        manufacturer: "Logitech",
        serial: "",
        speed_mbps: 12,
        max_power_ma: 98,
        version: "2.00",
        device_class: 0x00,
        bus_num: 5,
        dev_num: 4,
        removable: "removable",
        interfaces: &[InterfaceFixture {
            number: 0,
            class: 0x03,
            sub_class: 1,
            protocol: 1,
            driver: "usbhid",
        }],
    }
    .write(root.path());

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let snap = mgr.snapshot();
    let root_hub = snap
        .usb_devices
        .iter()
        .find(|d| d.bus_port == "usb5")
        .unwrap();
    let outer_hub = root_hub
        .children
        .iter()
        .find(|d| d.bus_port == "5-2")
        .unwrap();
    let inner_hub = outer_hub
        .children
        .iter()
        .find(|d| d.bus_port == "5-2.1")
        .unwrap();
    assert_eq!(inner_hub.children.len(), 1);
    assert_eq!(inner_hub.children[0].bus_port, "5-2.1.1");
    assert_eq!(inner_hub.children[0].product, "USB Receiver");
}

#[test]
fn missing_root_yields_empty_manager() {
    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/whatcable/path"));
    mgr.refresh();
    assert!(mgr.devices().is_empty());
    assert!(mgr.usb_devices().is_empty());
}
