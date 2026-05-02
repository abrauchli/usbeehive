#![cfg(feature = "sysfs")]

//! Integration tests: hand-crafted Type-C / USB-PD scenarios.

mod fixture_builder;
use fixture_builder::*;

use whatcable::diagnostic::Bottleneck;
use whatcable::pd::{CableCurrent, CableSpeed};
use whatcable::summary::Status;
use whatcable::{DeviceManager, Sysfs};

#[test]
fn empty_typec_port_renders_disconnected() {
    let root = TempRoot::new("tc-empty");
    write_typec_port(
        root.path(),
        "port0",
        &[
            ("data_role", "[host] device"),
            ("power_role", "[source] sink"),
            ("port_type", "dual"),
            ("orientation", "unknown"),
        ],
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let snap = mgr.snapshot();
    assert_eq!(snap.typec_ports.len(), 1);
    assert!(snap.typec_ports[0].partner.is_none());
    assert!(snap.typec_ports[0].cable.is_none());
    assert_eq!(snap.summaries.len(), 1);
    assert_eq!(snap.summaries[0].status, Status::Empty);
    assert_eq!(snap.summaries[0].subtitle, "Nothing connected");
}

#[test]
fn typec_charging_with_pps_charger_and_5a_active_cable() {
    let root = TempRoot::new("tc-charging");

    write_typec_port(
        root.path(),
        "port0",
        &[
            ("data_role", "host [device]"),
            ("power_role", "source [sink]"),
            ("port_type", "dual"),
            ("orientation", "normal"),
            ("usb_power_delivery_revision", "3.1"),
            ("power_operation_mode", "USB_POWER_DELIVERY"),
        ],
    );

    // Partner: peripheral, Apple VID
    write_typec_partner(
        root.path(),
        "port0",
        "device",
        &[
            // ID Header: peripheral (UFP=2), Apple VID 0x05AC
            ("id_header", (2u32 << 27) | 0x05AC),
            ("cert_stat", 0),
            ("product", 0x12A8_0001),
        ],
    );

    // Cable: active, Gen2, 5A, 50V (250W max)
    let id_hdr = (4u32 << 27) | 0x05AC; // ActiveCable, Apple
    let cable_vdo = 2u32 | (2 << 5) | (3 << 9); // Gen2, 5A, 50V
    write_typec_cable(
        root.path(),
        "port0",
        "active",
        "type-c",
        &[
            ("id_header", id_hdr),
            ("cert_stat", 0),
            ("product", 0),
            ("product_type_vdo1", cable_vdo),
        ],
    );

    // PD port: 100W charger advertising 5V/9V/15V/20V + PPS profile
    write_pd_port(
        root.path(),
        "pd0",
        0,
        &[
            PdoFixture {
                voltage_mv: 5000,
                current_ma: 3000,
                power_mw: 15_000,
                kind: "fixed_supply",
                min_voltage_mv: 0,
                max_voltage_mv: 0,
            },
            PdoFixture {
                voltage_mv: 9000,
                current_ma: 3000,
                power_mw: 27_000,
                kind: "fixed_supply",
                min_voltage_mv: 0,
                max_voltage_mv: 0,
            },
            PdoFixture {
                voltage_mv: 15_000,
                current_ma: 3000,
                power_mw: 45_000,
                kind: "fixed_supply",
                min_voltage_mv: 0,
                max_voltage_mv: 0,
            },
            PdoFixture {
                voltage_mv: 20_000,
                current_ma: 5000,
                power_mw: 100_000,
                kind: "fixed_supply",
                min_voltage_mv: 0,
                max_voltage_mv: 0,
            },
            PdoFixture {
                voltage_mv: 0,
                current_ma: 5000,
                power_mw: 0,
                kind: "programmable_supply (pps)",
                min_voltage_mv: 3300,
                max_voltage_mv: 21_000,
            },
        ],
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let snap = mgr.snapshot();
    assert_eq!(snap.typec_ports.len(), 1);
    assert_eq!(snap.pd_ports.len(), 1);
    assert_eq!(snap.summaries.len(), 1);

    let s = &snap.summaries[0];
    assert_eq!(s.status, Status::Charging);
    assert!(s.subtitle.contains("Apple"));
    let cable = s.cable.as_ref().unwrap();
    assert_eq!(cable.speed, Some(CableSpeed::Usb32Gen2));
    assert_eq!(cable.current_rating, Some(CableCurrent::FiveAmp));
    assert_eq!(cable.max_watts, 250);
    assert!(cable.is_active);

    // 100W charger + 250W cable = no bottleneck
    let diag = s.charging_diag.as_ref().unwrap();
    assert_eq!(diag.bottleneck, Bottleneck::Fine);

    // PPS profile decoded
    let pd = s.power_delivery.as_ref().unwrap();
    let pps = pd
        .source_capabilities
        .iter()
        .find(|p| matches!(p.r#type, whatcable::power::PdoType::Pps))
        .expect("PPS PDO present");
    assert_eq!(pps.voltage_mv, 3300);
    assert_eq!(pps.max_voltage_mv, 21_000);
    assert_eq!(pps.voltage_label(), "3.3-21.0V");

    // 4 fixed PDOs come back with the advertised wattage rounded to the watt
    let fixed: Vec<_> = pd
        .source_capabilities
        .iter()
        .filter(|p| matches!(p.r#type, whatcable::power::PdoType::FixedSupply))
        .collect();
    assert_eq!(fixed.len(), 4);
    assert_eq!(fixed.iter().map(|p| p.power_mw).max(), Some(100_000));
}

#[test]
fn cable_bottleneck_flags_warning() {
    let root = TempRoot::new("tc-cable-limit");
    write_typec_port(
        root.path(),
        "port0",
        &[
            ("data_role", "host [device]"),
            ("power_role", "[source] sink"),
        ],
    );
    write_typec_partner(root.path(), "port0", "device", &[]);

    // Passive cable rated 60W (3A, 20V) — counterpart for a 100W charger.
    let cable_vdo = 1u32 | (1 << 5); // Gen1, 3A, 20V (default)
    write_typec_cable(
        root.path(),
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
        root.path(),
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

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let s = &mgr.snapshot().summaries[0];
    let diag = s.charging_diag.as_ref().unwrap();
    assert_eq!(diag.bottleneck, Bottleneck::CableLimit);
    assert!(diag.is_warning);
    assert!(diag.detail.contains("60W"));
    assert!(diag.detail.contains("100W"));
}

#[test]
fn pd_port_without_typec_partner_yields_no_summary_match() {
    // PD-only without any Type-C port → no summary (we need a port to bind to).
    let root = TempRoot::new("tc-pd-orphan");
    write_pd_port(
        root.path(),
        "pd0",
        -1,
        &[PdoFixture {
            voltage_mv: 5000,
            current_ma: 3000,
            power_mw: 15_000,
            kind: "fixed_supply",
            min_voltage_mv: 0,
            max_voltage_mv: 0,
        }],
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();
    assert_eq!(mgr.snapshot().pd_ports.len(), 1);
    assert!(mgr.snapshot().summaries.is_empty());
}

#[test]
fn json_roundtrip_for_typec_port() {
    // Make sure the captured types serialize cleanly under serde_json.
    // (We dev-depend on serde_json via the test harness.)
    let root = TempRoot::new("tc-json");
    write_typec_port(root.path(), "port0", &[("data_role", "host [device]")]);
    write_typec_partner(
        root.path(),
        "port0",
        "device",
        &[("id_header", (2u32 << 27) | 0x05AC)],
    );

    let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root(root.path()));
    mgr.refresh();

    let json = serde_json::to_string(&mgr.snapshot().summaries[0]).unwrap();
    assert!(json.contains("\"category\":\"TypeCPort\""));
    assert!(json.contains("\"vendor_id\":1452")); // 0x05AC
}
