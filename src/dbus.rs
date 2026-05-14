// `zbus`'s `#[interface]` macro generates trait-impl glue for every method
// without forwarding our doc comments to the synthesized `&self` shims, so
// `missing_docs` fires on lines we cannot annotate. Silence it for the
// module — the public surface is still documented above each method.
#![allow(missing_docs)]

//! Optional D-Bus interface for `usbeehive`.
//!
//! Compiled only when the `dbus` Cargo feature is enabled. Hosts the
//! `org.usbeehive.Devices2` interface backing the `usbeehived` daemon, plus
//! the wire types ([`DeviceEntry`], [`PowerEntry`], [`DiagnosticEntry`])
//! clients receive.
//!
//! The interface state is held behind an `Arc<Mutex<…>>` so the daemon's
//! background hot-plug thread (running [`crate::watch::run_loop`]) can
//! refresh the snapshot and emit signals while D-Bus method calls keep
//! reading.
//!
//! # Wire surface
//!
//! Bus name: `org.usbeehive.Devices`
//! Object path: `/org/usbeehive/Devices`
//! Interface: `org.usbeehive.Devices2`
//!
//! ## `ListDevices` element shape
//!
//! Per-entry signature: `a(ssssssssss qq s a(ss) i u s (uus) (bsssb))`.
//!
//! | Pos | Field | Type | Notes |
//! |---|---|---|---|
//! | 1 | `id` | `s` | Stable identifier — `typec:<port>` / `usb:<bus_port>`. |
//! | 2 | `category` | `s` | `UsbDevice` \| `TypeCPort` \| `Hub`. |
//! | 3 | `device_class` | `s` | Coarse classification (see [`DeviceClass`]). `Unknown` for Type-C ports. |
//! | 4 | `device_subclass` | `s` | Advisory fine-grain hint (`webcam`, `capture`, `sd_reader`, …). Empty by default. Adding values is non-breaking. |
//! | 5 | `status` | `s` | `Empty` \| `Connected` \| `Charging` \| `Sourcing`. |
//! | 6 | `headline` | `s` | Single-line display title (English prose). |
//! | 7 | `subtitle` | `s` | Single-line display subtitle (English prose). |
//! | 8 | `icon` | `s` | freedesktop icon name. |
//! | 9 | `vendor` | `s` | Manufacturer string (descriptor + vendor-DB fallback). |
//! | 10 | `product` | `s` | Raw USB `iProduct` descriptor. |
//! | 11 | `vendor_id` | `q` | `idVendor` (uint16). Zero for non-USB. |
//! | 12 | `product_id` | `q` | `idProduct` (uint16). Zero for non-USB. |
//! | 13 | `primary_driver` | `s` | Kernel driver bound to the device's first interface. Empty when unbound. |
//! | 14 | `properties` | `a(ss)` | `(machine_key, value)` pairs. Adding keys is non-breaking; renaming/removing requires an interface bump. |
//! | 15 | `port_number` | `i` | Type-C port number, `-1` otherwise. |
//! | 16 | `link_speed_mbps` | `u` | Negotiated USB link speed in Mbps, `0` if unknown. |
//! | 17 | `usb_version` | `s` | Canonical short form (`"2.0"`, `"3.2"`, `"4.0"`). Empty if unknown. |
//! | 18 | `power` | `(uus)` | `(power_in_mw, power_out_mw, power_role)`. `power_in_mw > 0` ⟺ port is actively sinking PD power. |
//! | 19 | `charging_diag` | `(bsssb)` | `(present, bottleneck, summary, detail, is_warning)`. `present == false` on every non-`Charging` entry. |
//!
//! ## Methods, properties, signals
//!
//! | Member | Signature | Notes |
//! |---|---|---|
//! | `ListDevices` | `() → a(ssssssssssqqsa(ss)ius(uus)(bsssb))` | One [`DeviceEntry`] per summary. |
//! | `ListPorts` | `() → ai` | Type-C `port_number`s currently exposed. |
//! | `Diagnose` | `(i) → (bsssb)` | Same shape as the per-entry `charging_diag`. `present == false` when no diagnostic is available for the port. |
//! | `SnapshotJson` | `() → s` | Full structured snapshot serialised with `serde_json`. |
//! | `Refresh` | `() → u` | Force a re-enumeration; returns the new summary count. |
//! | `Version` (property) | `s` | The crate version string. |
//! | `DeviceCount` (property) | `u` | Number of summaries in the latest snapshot. |
//! | `DeviceAdded` (signal) | `(ss)` | `(id, headline)` for a newly attached device. |
//! | `DeviceRemoved` (signal) | `s` | `id` of a device that disappeared. |
//! | `CapabilityDegraded` (signal) | `(iss)` | `(port_number, summary, detail)` when a port's charging diagnostic newly raises `is_warning`. |
//! | `CapabilityRestored` (signal) | `i` | `port_number` whose previous warning has cleared. |
//!
//! # Migrating from `Devices1`
//!
//! Hard cut — `Devices1` is gone, no alias. Clients must:
//!
//! 1. Update the proxy/`Connect` call to use `org.usbeehive.Devices2`.
//! 2. Replace `bullets: as` parsing with `properties: a(ss)` lookups by
//!    machine key. The label vocabulary is documented in the CHANGELOG
//!    migration entry.
//! 3. Replace `Diagnose() → (sssb)` callers with `(bsssb)` (the leading
//!    `present: bool` was added for unambiguous absence — `Fine` is a
//!    non-empty bottleneck, empty-string-as-absent would conflate them).
//! 4. Read the new structured fields directly: `link_speed_mbps` for
//!    speed, `usb_version` for version, `power.power_role` for direction,
//!    `power.power_in_mw` for inbound watts, `charging_diag.is_warning`
//!    for "this entry has a user-actionable problem", and `device_class`
//!    for icon routing.
//!
//! ## Enum extensibility convention
//!
//! Adding a new `category` / `device_class` / `device_subclass` / `status`
//! / `power_role` / `bottleneck` variant is **non-breaking**. Clients MUST
//! treat unrecognised string values as `Unknown` and fall back to
//! category-based behaviour. Removing or renaming variants requires
//! an interface bump.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::Type;

use crate::diagnostic::ChargingDiagnostic;
use crate::summary::{DeviceSummary, PowerSummary};
use crate::sysfs::manager::{DeviceManager, Snapshot, SnapshotDiff};

/// Power flow summary wire type.
///
/// Carries the same data as [`crate::summary::PowerSummary`]; enum fields
/// are flattened to their Debug-variant name as a UTF-8 string for
/// dbus-monitor friendliness.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct PowerEntry {
    /// Inbound power (we are sinking) in milliwatts. Zero when not sinking.
    pub power_in_mw: u32,
    /// Outbound power (we are sourcing) in milliwatts. Zero when not sourcing.
    pub power_out_mw: u32,
    /// `Source` | `Sink` | `DualRole` | `Unknown`.
    pub power_role: String,
}

impl From<&PowerSummary> for PowerEntry {
    fn from(p: &PowerSummary) -> Self {
        PowerEntry {
            power_in_mw: p.power_in_mw,
            power_out_mw: p.power_out_mw,
            power_role: format!("{:?}", p.power_role),
        }
    }
}

/// Per-entry charging diagnostic. Same shape as `Diagnose(port)`.
///
/// `present == false` indicates the port has no diagnostic to report
/// (non-charging entry, no PD source advertised, …); the remaining
/// string fields are empty and `is_warning` is `false`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct DiagnosticEntry {
    /// `true` when a diagnostic was computed.
    pub present: bool,
    /// Bottleneck variant name (`"NoCharger"`, `"ChargerLimit"`,
    /// `"CableLimit"`, `"DeviceLimit"`, `"Fine"`).
    pub bottleneck: String,
    /// One-line headline.
    pub summary: String,
    /// Optional second-line detail.
    pub detail: String,
    /// `true` when the bottleneck is user-actionable (e.g. swap the cable).
    pub is_warning: bool,
}

impl From<Option<&ChargingDiagnostic>> for DiagnosticEntry {
    fn from(diag: Option<&ChargingDiagnostic>) -> Self {
        match diag {
            Some(d) => DiagnosticEntry {
                present: true,
                bottleneck: format!("{:?}", d.bottleneck),
                summary: d.summary.clone(),
                detail: d.detail.clone(),
                is_warning: d.is_warning,
            },
            None => DiagnosticEntry::default(),
        }
    }
}

/// One device or Type-C port as published over D-Bus.
///
/// Serialisable mirror of [`DeviceSummary`]. See the module docs for the
/// per-field semantics and the migration guide from `Devices1`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceEntry {
    /// Stable identifier.
    pub id: String,
    /// `"UsbDevice"` | `"TypeCPort"` | `"Hub"`.
    pub category: String,
    /// Coarse classification. `"Unknown"` for Type-C ports.
    pub device_class: String,
    /// Advisory fine-grain hint, empty when unset.
    pub device_subclass: String,
    /// Connection / power flow state.
    pub status: String,
    /// Display title.
    pub headline: String,
    /// Display subtitle.
    pub subtitle: String,
    /// freedesktop icon name.
    pub icon: String,
    /// Manufacturer string (descriptor + vendor-DB fallback).
    pub vendor: String,
    /// Raw USB `iProduct` string.
    pub product: String,
    /// `idVendor`. Zero for non-USB entries.
    pub vendor_id: u16,
    /// `idProduct`. Zero for non-USB entries.
    pub product_id: u16,
    /// Kernel driver bound to the first interface. Empty when unbound.
    pub primary_driver: String,
    /// `(machine_key, value)` property pairs.
    pub properties: Vec<(String, String)>,
    /// Type-C port number when applicable, otherwise `-1`.
    pub port_number: i32,
    /// Negotiated link speed in Mbps. Zero when unknown.
    pub link_speed_mbps: u32,
    /// Canonical USB version short form (`"2.0"`, `"3.2"`, `"4.0"`).
    pub usb_version: String,
    /// Power flow summary.
    pub power: PowerEntry,
    /// Charging diagnostic. `present == false` on every non-`Charging` entry.
    pub charging_diag: DiagnosticEntry,
}

impl From<&DeviceSummary> for DeviceEntry {
    fn from(s: &DeviceSummary) -> Self {
        let port_number = s.typec_port.as_ref().map(|p| p.port_number).unwrap_or(-1);
        DeviceEntry {
            id: s.id(),
            category: format!("{:?}", s.category),
            device_class: format!("{:?}", s.device_class),
            device_subclass: s.device_subclass.clone(),
            status: format!("{:?}", s.status),
            headline: s.headline.clone(),
            subtitle: s.subtitle.clone(),
            icon: s.icon.clone(),
            vendor: s.vendor.clone(),
            product: s.product.clone(),
            vendor_id: s.vendor_id,
            product_id: s.product_id,
            primary_driver: s.primary_driver.clone(),
            properties: s.properties.clone(),
            port_number,
            link_speed_mbps: s.link_speed_mbps,
            usb_version: s.usb_version.clone(),
            power: PowerEntry::from(&s.power),
            charging_diag: DiagnosticEntry::from(s.charging_diag.as_ref()),
        }
    }
}

/// Mutable state shared between the hot-plug loop and D-Bus dispatch.
pub struct State {
    /// Underlying device manager that owns the live [`Snapshot`].
    pub manager: DeviceManager,
    /// Snapshot captured at the previous refresh — diff baseline.
    pub previous: Snapshot,
}

impl State {
    /// Build a fresh state around `manager`. The first [`Self::refresh`]
    /// call will treat every device as `added`.
    pub fn new(manager: DeviceManager) -> Self {
        State {
            manager,
            previous: Snapshot::default(),
        }
    }

    /// Re-enumerate `/sys`, advance `previous`, and return the diff.
    pub fn refresh(&mut self) -> SnapshotDiff {
        self.manager.refresh();
        let diff = self.manager.snapshot().diff(&self.previous);
        self.previous = self.manager.snapshot().clone();
        diff
    }

    /// Headline for the summary identified by `id`, or `id` itself when no
    /// matching summary is currently held.
    pub fn headline_for(&self, id: &str) -> String {
        self.manager
            .devices()
            .iter()
            .find(|s| s.id() == id)
            .map(|s| s.headline.clone())
            .unwrap_or_else(|| id.to_string())
    }
}

/// `org.usbeehive.Devices2` interface implementation.
pub struct DevicesIface {
    /// Shared state backing every method call.
    pub state: Arc<Mutex<State>>,
}

impl DevicesIface {
    /// Helper for tests / in-process clients. Returns the same vector that
    /// `ListDevices` returns over the wire.
    pub fn snapshot_entries(&self) -> Vec<DeviceEntry> {
        let state = self.state.lock().expect("state mutex poisoned");
        state
            .manager
            .devices()
            .iter()
            .map(DeviceEntry::from)
            .collect()
    }

    /// Helper for tests / in-process clients. Returns the same payload as
    /// `Diagnose(port)` over the wire.
    pub fn diagnose_port(&self, port_number: i32) -> DiagnosticEntry {
        let state = self.state.lock().expect("state mutex poisoned");
        let summary = state.manager.devices().iter().find(|s| {
            s.typec_port
                .as_ref()
                .is_some_and(|p| p.port_number == port_number)
        });
        DiagnosticEntry::from(summary.and_then(|s| s.charging_diag.as_ref()))
    }
}

#[interface(name = "org.usbeehive.Devices2")]
impl DevicesIface {
    /// Return one [`DeviceEntry`] per summary in the latest snapshot.
    fn list_devices(&self) -> Vec<DeviceEntry> {
        self.snapshot_entries()
    }

    /// Type-C `port_number`s currently exposed (in snapshot order).
    fn list_ports(&self) -> Vec<i32> {
        let state = self.state.lock().expect("state mutex poisoned");
        state
            .manager
            .devices()
            .iter()
            .filter_map(|s| s.typec_port.as_ref().map(|p| p.port_number))
            .collect()
    }

    /// Charging diagnostic for `port_number`. Returns
    /// `DiagnosticEntry { present: false, .. }` when no diagnostic exists.
    fn diagnose(&self, port_number: i32) -> DiagnosticEntry {
        self.diagnose_port(port_number)
    }

    /// Full structured snapshot serialised with `serde_json`. Mirrors the
    /// CLI's `--json` output so any client can deserialise it without
    /// re-deriving D-Bus types for every nested field.
    fn snapshot_json(&self) -> zbus::fdo::Result<String> {
        let state = self.state.lock().expect("state mutex poisoned");
        serde_json::to_string(state.manager.devices())
            .map_err(|e| zbus::fdo::Error::Failed(format!("serde_json: {e}")))
    }

    /// Force a re-enumeration. Returns the new summary count.
    fn refresh(&mut self) -> u32 {
        let mut state = self.state.lock().expect("state mutex poisoned");
        let _ = state.refresh();
        state.manager.devices().len() as u32
    }

    /// Crate version, e.g. `"0.5.1"`.
    #[zbus(property)]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Number of summaries in the latest snapshot.
    #[zbus(property)]
    fn device_count(&self) -> u32 {
        self.state
            .lock()
            .expect("state mutex poisoned")
            .manager
            .devices()
            .len() as u32
    }

    /// Emitted when a new device or port becomes visible.
    #[zbus(signal)]
    pub async fn device_added(
        emitter: &SignalEmitter<'_>,
        id: String,
        headline: String,
    ) -> zbus::Result<()>;

    /// Emitted when a previously-visible device or port disappears.
    #[zbus(signal)]
    pub async fn device_removed(emitter: &SignalEmitter<'_>, id: String) -> zbus::Result<()>;

    /// Emitted when a port's charging diagnostic newly raises `is_warning`.
    /// Payload duplicates `entry.charging_diag.{summary,detail}` so the
    /// signal is self-contained for notification UX.
    #[zbus(signal)]
    pub async fn capability_degraded(
        emitter: &SignalEmitter<'_>,
        port_number: i32,
        summary: String,
        detail: String,
    ) -> zbus::Result<()>;

    /// Emitted when a port that was previously raising `is_warning` no
    /// longer is — the bottleneck cleared.
    #[zbus(signal)]
    pub async fn capability_restored(
        emitter: &SignalEmitter<'_>,
        port_number: i32,
    ) -> zbus::Result<()>;
}

/// Bus name the daemon requests on the session bus.
pub const BUS_NAME: &str = "org.usbeehive.Devices";
/// Object path the [`DevicesIface`] is published at.
pub const OBJECT_PATH: &str = "/org/usbeehive/Devices";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::{Category, PowerRole, Status};
    use crate::typec::{TypeCPartner, TypeCPort};
    use crate::usb::UsbDevice;

    #[test]
    fn diagnostic_entry_default_is_absent() {
        let d = DiagnosticEntry::default();
        assert!(!d.present);
        assert_eq!(d.summary, "");
        assert!(!d.is_warning);
    }

    #[test]
    fn power_entry_serializes_enum_as_string() {
        let p = PowerSummary {
            power_in_mw: 60_000,
            power_out_mw: 0,
            power_role: PowerRole::Sink,
        };
        let e = PowerEntry::from(&p);
        assert_eq!(e.power_in_mw, 60_000);
        assert_eq!(e.power_role, "Sink");
    }

    #[test]
    fn device_entry_from_typec_summary_carries_structured_fields() {
        let port = TypeCPort {
            port_number: 7,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, None);
        let e = DeviceEntry::from(&s);
        assert_eq!(e.port_number, 7);
        assert_eq!(e.category, "TypeCPort");
        assert_eq!(e.device_class, "Unknown");
        assert_eq!(e.status, format!("{:?}", Status::Connected));
        assert_eq!(e.id, "typec:");
        assert!(!e.charging_diag.present);
        // No PD → power_role falls through to capability or Unknown.
        assert!(matches!(
            e.power.power_role.as_str(),
            "Unknown" | "DualRole"
        ));
    }

    #[test]
    fn device_entry_from_usb_summary_marks_no_port_number() {
        let usb = UsbDevice {
            bus_port: "1-1".into(),
            product: "thing".into(),
            vendor_id: 0x1234,
            product_id: 0x5678,
            speed: 480,
            version: "2.10".into(),
            ..Default::default()
        };
        let s = DeviceSummary::from_usb_device(&usb);
        let e = DeviceEntry::from(&s);
        assert_eq!(e.port_number, -1);
        assert_eq!(e.id, "usb:1-1");
        assert_eq!(e.headline, "thing");
        assert_eq!(e.vendor_id, 0x1234);
        assert_eq!(e.product_id, 0x5678);
        assert_eq!(e.link_speed_mbps, 480);
        assert_eq!(e.usb_version, "2.1");
        assert!(!e.charging_diag.present);
        // USB plain device on a hub — power flow is zero, role Unknown.
        assert_eq!(e.power.power_in_mw, 0);
        assert_eq!(e.power.power_role, "Unknown");
    }

    #[test]
    fn device_entry_carries_charging_diag_when_present() {
        use crate::diagnostic::{Bottleneck, ChargingDiagnostic};
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let mut s = DeviceSummary::from_typec_port(&port, None, None);
        s.status = Status::Charging;
        s.charging_diag = Some(ChargingDiagnostic {
            bottleneck: Bottleneck::CableLimit,
            summary: "Cable is limiting charging speed".into(),
            detail: "Cable rated for 60W, but charger can deliver 100W".into(),
            is_warning: true,
        });
        let e = DeviceEntry::from(&s);
        assert!(e.charging_diag.present);
        assert_eq!(e.charging_diag.bottleneck, "CableLimit");
        assert_eq!(e.charging_diag.summary, "Cable is limiting charging speed");
        assert!(e.charging_diag.detail.contains("60W"));
        assert!(e.charging_diag.is_warning);
    }

    #[test]
    fn empty_state_yields_empty_iface_responses() {
        let _category_unused = Category::UsbDevice;
        let manager = DeviceManager::with_sysfs(crate::Sysfs::with_root("/no/such/usbeehive"));
        let state = Arc::new(Mutex::new(State::new(manager)));
        let iface = DevicesIface { state };
        assert!(iface.snapshot_entries().is_empty());
        let d = iface.diagnose_port(0);
        assert!(!d.present);
    }
}
