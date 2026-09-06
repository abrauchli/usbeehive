#![cfg(feature = "sysfs")]

//! Integration tests: hand-crafted sysfs trees → DeviceManager snapshots.

mod fixture_builder;
use fixture_builder::*;

use usbeehive::{DeviceManager, Sysfs};

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
    assert_eq!(kb.primary_driver, "usbhid");

    let storage = snap
        .summaries
        .iter()
        .find(|s| s.headline == "DataTraveler 3.0")
        .unwrap();
    assert!(storage.subtitle.contains("Kingston"));
    assert!(storage.subtitle.contains("Mass Storage"));
    assert!(storage
        .properties
        .iter()
        .any(|(k, v)| k == "serial" && v == "ABCD1234"));
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
    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/usbeehive/path"));
    mgr.refresh();
    assert!(mgr.devices().is_empty());
    assert!(mgr.usb_devices().is_empty());
}

// ---------------------------------------------------------------------
// BOS descriptors — capability vs negotiated data rate.
// ---------------------------------------------------------------------

/// Real blob captured from a "USB 10/100/1000 LAN" adapter: advertises
/// SuperSpeed (`wSpeedsSupported == 0x000E`) but declares full functionality
/// already at High Speed (`bFunctionalitySupport == 2`).
const BOS_LAN_SUPERSPEED: &[u8] = &[
    0x05, 0x0f, 0x16, 0x00, 0x02, //
    0x07, 0x10, 0x02, 0x06, 0x00, 0x00, 0x00, //
    0x0a, 0x10, 0x03, 0x02, 0x0e, 0x00, 0x02, 0x0a, 0xff, 0x07,
];

fn lan_fixture(bus_port: &str, speed_mbps: u32) -> UsbDeviceFixture<'_> {
    UsbDeviceFixture {
        bus_port,
        vendor: 0x0B95,
        product: 0x1790,
        product_name: "USB 10/100/1000 LAN",
        manufacturer: "ASIX",
        serial: "",
        speed_mbps,
        max_power_ma: 250,
        version: "2.10",
        device_class: 0x00,
        bus_num: 5,
        dev_num: 4,
        removable: "removable",
        interfaces: &[],
    }
}

fn prop<'a>(s: &'a usbeehive::DeviceSummary, key: &str) -> Option<&'a str> {
    s.properties
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn bos_capability_surfaces_without_flagging_a_vendor_declared_floor() {
    let root = TempRoot::new("usb-bos-floor");
    lan_fixture("5-2", 480).write(root.path());
    write_bos(root.path(), "5-2", BOS_LAN_SUPERSPEED);

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let dev = mgr
        .usb_devices()
        .iter()
        .find(|d| d.bus_port == "5-2")
        .expect("device enumerated");
    let bos = dev.bos.as_ref().expect("BOS decoded");
    assert!(!bos.truncated);
    assert_eq!(bos.max_capable_mbps(), 5_000);
    assert_eq!(bos.functional_floor_mbps(), Some(480));
    assert!(!dev.bos_suppressed_by_quirk());

    let s = mgr
        .devices()
        .iter()
        .find(|s| s.id() == "usb:5-2")
        .expect("summary");
    let a = s.data_rate.as_ref().expect("assessment");
    assert_eq!(a.verdict, usbeehive::DataRateVerdict::BelowCapability);
    assert!(!a.is_warning);

    assert_eq!(prop(s, "usb_capable_speed_mbps"), Some("5000"));
    assert_eq!(prop(s, "usb_capable_speed"), Some("SuperSpeed 5 Gbps"));
    assert_eq!(prop(s, "usb_functional_floor_mbps"), Some("480"));
    assert_eq!(prop(s, "usb_link_verdict"), Some("BelowCapability"));
    // The whole point: no warning key when the vendor's own floor is met.
    assert_eq!(prop(s, "usb_link_degraded"), None);
}

#[test]
fn bos_capability_flags_a_link_below_the_declared_floor() {
    let root = TempRoot::new("usb-bos-degraded");
    lan_fixture("5-2", 12).write(root.path());
    write_bos(root.path(), "5-2", BOS_LAN_SUPERSPEED);

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let s = mgr
        .devices()
        .iter()
        .find(|s| s.id() == "usb:5-2")
        .expect("summary");
    let a = s.data_rate.as_ref().expect("assessment");
    assert_eq!(a.verdict, usbeehive::DataRateVerdict::Degraded);
    assert!(a.is_warning);
    assert!(!a.summary.is_empty());
    assert_eq!(prop(s, "usb_link_verdict"), Some("Degraded"));
    assert_eq!(prop(s, "usb_link_degraded"), Some("true"));
}

#[test]
fn absent_bos_yields_no_assessment_and_no_keys() {
    let root = TempRoot::new("usb-bos-absent");
    lan_fixture("5-2", 480).write(root.path());
    // Deliberately no write_bos — the USB 2.0-only case.

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let dev = mgr
        .usb_devices()
        .iter()
        .find(|d| d.bus_port == "5-2")
        .unwrap();
    assert!(dev.bos.is_none());
    assert!(dev.data_rate().is_none());
    assert!(!dev.bos_suppressed_by_quirk());

    let s = mgr.devices().iter().find(|s| s.id() == "usb:5-2").unwrap();
    assert!(s.data_rate.is_none());
    for key in [
        "usb_capable_speed_mbps",
        "usb_capable_speed",
        "usb_link_verdict",
        "usb_link_degraded",
        "usb_bos_suppressed",
    ] {
        assert_eq!(prop(s, key), None, "unexpected key {key}");
    }
}

#[test]
fn quirk_suppressed_bos_is_unknown_not_a_shortfall() {
    let root = TempRoot::new("usb-bos-quirked");
    // A 480 Mbps link with no BOS file, but USB_QUIRK_NO_BOS (BIT(17)) set:
    // the kernel never read the BOS, so capability is unknown — the device
    // may well be SuperSpeed-capable.
    lan_fixture("5-2", 480).write(root.path());
    write_attr(
        &root.path().join("bus/usb/devices/5-2"),
        "quirks",
        "0x20000",
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let dev = mgr
        .usb_devices()
        .iter()
        .find(|d| d.bus_port == "5-2")
        .unwrap();
    assert_eq!(dev.quirks, usbeehive::usb::USB_QUIRK_NO_BOS);
    assert!(dev.bos.is_none());
    assert!(dev.bos_suppressed_by_quirk());
    // No BOS ⇒ no verdict ⇒ can never be reported as degraded.
    assert!(dev.data_rate().is_none());

    let s = mgr.devices().iter().find(|s| s.id() == "usb:5-2").unwrap();
    assert!(s.data_rate.is_none());
    assert_eq!(prop(s, "usb_bos_suppressed"), Some("true"));
    assert_eq!(prop(s, "usb_link_verdict"), None);
    assert_eq!(prop(s, "usb_link_degraded"), None);
}

#[test]
fn unrelated_quirk_does_not_mark_bos_suppressed() {
    let root = TempRoot::new("usb-bos-other-quirk");
    lan_fixture("5-2", 480).write(root.path());
    // USB_QUIRK_NO_LPM — real value observed on live hardware.
    write_attr(&root.path().join("bus/usb/devices/5-2"), "quirks", "0x400");
    write_bos(root.path(), "5-2", BOS_LAN_SUPERSPEED);

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let dev = mgr
        .usb_devices()
        .iter()
        .find(|d| d.bus_port == "5-2")
        .unwrap();
    assert_eq!(dev.quirks, usbeehive::usb::USB_QUIRK_NO_LPM);
    assert!(!dev.bos_suppressed_by_quirk());
    assert!(dev.bos.is_some());

    let s = mgr.devices().iter().find(|s| s.id() == "usb:5-2").unwrap();
    assert_eq!(prop(s, "usb_bos_suppressed"), None);
}

#[test]
fn malformed_bos_blob_never_panics_and_yields_no_verdict() {
    let root = TempRoot::new("usb-bos-malformed");
    lan_fixture("5-2", 480).write(root.path());
    // Valid BOS header, then a zero-bLength capability.
    write_bos(
        root.path(),
        "5-2",
        &[
            0x05, 0x0f, 0x0c, 0x00, 0x02, 0x00, 0x10, 0x02, 0x00, 0x00, 0x00, 0x00,
        ],
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let dev = mgr
        .usb_devices()
        .iter()
        .find(|d| d.bus_port == "5-2")
        .unwrap();
    let bos = dev.bos.as_ref().expect("header still decodes");
    assert!(bos.truncated);
    assert!(bos.capabilities.is_empty());

    let s = mgr.devices().iter().find(|s| s.id() == "usb:5-2").unwrap();
    let a = s
        .data_rate
        .as_ref()
        .expect("assessment present but Unknown");
    assert_eq!(a.verdict, usbeehive::DataRateVerdict::Unknown);
    assert!(!a.is_warning);
    assert_eq!(prop(s, "usb_link_verdict"), None);
    assert_eq!(prop(s, "usb_link_degraded"), None);
}

#[test]
fn data_rate_diff_reports_ids_not_port_numbers() {
    let good = TempRoot::new("usb-bos-diff-good");
    lan_fixture("5-2", 480).write(good.path());
    write_bos(good.path(), "5-2", BOS_LAN_SUPERSPEED);

    let bad = TempRoot::new("usb-bos-diff-bad");
    lan_fixture("5-2", 12).write(bad.path());
    write_bos(bad.path(), "5-2", BOS_LAN_SUPERSPEED);

    let mut healthy = DeviceManager::with_sysfs(Sysfs::with_root(good.path()));
    healthy.refresh();
    let mut degraded = DeviceManager::with_sysfs(Sysfs::with_root(bad.path()));
    degraded.refresh();

    let d = degraded.snapshot().diff(healthy.snapshot());
    assert_eq!(d.newly_rate_degraded, vec!["usb:5-2".to_string()]);
    assert!(d.rate_restored.is_empty());
    // The charging-side lists stay untouched — different concept entirely.
    assert!(d.newly_degraded.is_empty());
    assert!(d.resolved.is_empty());
    // A verdict flip is user-visible state, so DeviceChanged fires too.
    assert!(d.changed.contains(&"usb:5-2".to_string()));

    let back = healthy.snapshot().diff(degraded.snapshot());
    assert_eq!(back.rate_restored, vec!["usb:5-2".to_string()]);
    assert!(back.newly_rate_degraded.is_empty());
}
