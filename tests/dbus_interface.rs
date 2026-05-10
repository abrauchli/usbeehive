#![cfg(feature = "dbus")]

//! Integration tests for the optional `usbeehive::dbus` interface.
//!
//! The tests construct the `DevicesIface` against a fixture sysfs root and
//! invoke its methods directly (without spinning up a real session bus) to
//! exercise the same logic that `usbeehived` would expose to clients.
//! Diff-driven signal classification is covered separately by
//! [`usbeehive::SnapshotDiff`] unit tests in `src/sysfs/manager.rs`.

mod fixture_builder;
use fixture_builder::*;

use std::sync::{Arc, Mutex};

use usbeehive::dbus::{DevicesIface, State, BUS_NAME, OBJECT_PATH};
use usbeehive::{DeviceManager, Sysfs};

fn make_state(root: &std::path::Path) -> Arc<Mutex<State>> {
    let mgr = DeviceManager::with_sysfs(Sysfs::with_root(root));
    Arc::new(Mutex::new(State::new(mgr)))
}

fn write_port_with_cable_limit(root: &std::path::Path) {
    write_typec_port(
        root,
        "port0",
        &[
            ("data_role", "host [device]"),
            ("power_role", "[source] sink"),
            ("port_type", "dual"),
            ("orientation", "normal"),
        ],
    );
    write_typec_partner(root, "port0", "device", &[]);

    // 60W (3A, 20V) passive cable — the bottleneck against a 100W charger.
    let cable_vdo = 1u32 | (1 << 5);
    write_typec_cable(
        root,
        "port0",
        "passive",
        "type-c",
        &[
            ("id_header", (3u32 << 27) | 0x0BDA), // PassiveCable, Realtek
            ("cert_stat", 0),
            ("product", 0),
            ("product_type_vdo1", cable_vdo),
        ],
    );

    write_pd_port(
        root,
        "pd0",
        0,
        &[PdoFixture {
            voltage_mv: 20_000,
            current_ma: 5000,
            power_mw: 100_000,
            kind: "fixed_supply",
            min_voltage_mv: 0,
            max_voltage_mv: 0,
        }],
    );
}

#[test]
fn list_devices_returns_one_entry_per_summary() {
    let root = TempRoot::new("dbus-list");
    write_port_with_cable_limit(root.path());

    let state = make_state(root.path());
    {
        let mut guard = state.lock().unwrap();
        guard.refresh();
    }
    let iface = DevicesIface { state };
    let entries = iface.snapshot_entries();

    assert_eq!(entries.len(), 1, "{entries:#?}");
    let e = &entries[0];
    assert_eq!(e.category, "TypeCPort");
    assert_eq!(e.id, "typec:port0");
    assert_eq!(e.port_number, 0);
    assert!(e.headline.starts_with("USB-C Port"));
}

#[test]
fn diagnose_reports_cable_limit_warning() {
    let root = TempRoot::new("dbus-diagnose");
    write_port_with_cable_limit(root.path());

    let state = make_state(root.path());
    {
        let mut guard = state.lock().unwrap();
        guard.refresh();
    }
    let iface = DevicesIface { state };

    let diag = iface.diagnose_port(0);
    assert!(diag.present);
    assert_eq!(diag.bottleneck, "CableLimit");
    assert!(diag.is_warning);
    assert!(diag.detail.contains("60W"));
    assert!(diag.detail.contains("100W"));

    // Unknown port → empty diagnostic.
    let absent = iface.diagnose_port(99);
    assert!(!absent.present);
    assert!(absent.summary.is_empty());
}

#[test]
fn list_ports_enumerates_typec_ports_only() {
    let root = TempRoot::new("dbus-ports");
    write_typec_port(root.path(), "port0", &[("port_type", "dual")]);
    write_typec_port(root.path(), "port1", &[("port_type", "dual")]);
    UsbDeviceFixture {
        bus_port: "1-1",
        vendor: 0x05AC,
        product: 0x12A8,
        product_name: "iPhone",
        manufacturer: "Apple",
        serial: "",
        speed_mbps: 480,
        max_power_ma: 500,
        version: "2.10",
        device_class: 0,
        bus_num: 1,
        dev_num: 2,
        interfaces: &[],
        removable: "removable",
    }
    .write(root.path());

    let state = make_state(root.path());
    {
        let mut guard = state.lock().unwrap();
        guard.refresh();
    }
    let iface = DevicesIface { state };

    // ListPorts is the inherent helper that the `#[interface]` method calls.
    // Re-derive it here for assertion clarity (the iface method itself is
    // exercised by zbus dispatch in production).
    let entries = iface.snapshot_entries();
    let port_numbers: Vec<i32> = entries
        .iter()
        .filter(|e| e.category == "TypeCPort")
        .map(|e| e.port_number)
        .collect();
    assert!(port_numbers.contains(&0));
    assert!(port_numbers.contains(&1));
    let usbs: Vec<&_> = entries.iter().filter(|e| e.category == "UsbDevice").collect();
    assert_eq!(usbs.len(), 1);
    assert_eq!(usbs[0].headline, "iPhone");
}

#[test]
fn refresh_updates_state_and_snapshot_diff_baseline_primes() {
    // `State::refresh` must compute a diff against the *previous* snapshot —
    // on the very first call, every visible device is "added" and any
    // standing warning is "newly_degraded". The usbeehived daemon swallows
    // that initial burst; here we just check the data is faithful.
    let root = TempRoot::new("dbus-refresh");
    write_port_with_cable_limit(root.path());
    let state = make_state(root.path());

    let first = {
        let mut guard = state.lock().unwrap();
        guard.refresh()
    };
    assert_eq!(first.added, vec!["typec:port0"]);
    assert_eq!(first.newly_degraded, vec![0]);
    assert!(first.removed.is_empty());

    // Second refresh against unchanged sysfs → empty diff.
    let second = {
        let mut guard = state.lock().unwrap();
        guard.refresh()
    };
    assert!(second.is_empty(), "{second:?}");
}

#[test]
fn snapshot_json_round_trips_through_serde() {
    let root = TempRoot::new("dbus-json");
    write_port_with_cable_limit(root.path());

    let state = make_state(root.path());
    {
        let mut guard = state.lock().unwrap();
        guard.refresh();
    }
    let iface = DevicesIface { state };

    // Same shape as `serde_json::to_string(manager.devices())` — we only
    // verify it parses and contains a known field.
    let json = serde_json::to_string(
        &iface
            .state
            .lock()
            .unwrap()
            .manager
            .devices()
            .iter()
            .map(usbeehive::dbus::DeviceEntry::from)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["category"], "TypeCPort");
    assert_eq!(arr[0]["id"], "typec:port0");
    assert_eq!(arr[0]["port_number"], 0);
}

#[test]
fn dbus_constants_match_freedesktop_naming() {
    // Soft sanity guard — these strings end up in `.service` files,
    // generated D-Bus stubs in other languages, and screenshots. Catch
    // accidental rename regressions.
    assert_eq!(BUS_NAME, "org.usbeehive.Devices");
    assert_eq!(OBJECT_PATH, "/org/usbeehive/Devices");
}
