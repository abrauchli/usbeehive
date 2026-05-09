// `zbus`'s `#[interface]` macro generates trait-impl glue for every method
// without forwarding our doc comments to the synthesized `&self` shims, so
// `missing_docs` fires on lines we cannot annotate. Silence it for the
// module — the public surface is still documented above each method.
#![allow(missing_docs)]

//! Optional D-Bus interface for `whatcable`.
//!
//! Compiled only when the `dbus` Cargo feature is enabled. Hosts the
//! `org.whatcable.Devices1` interface backing the `whatcabled` daemon, plus
//! the wire types ([`DeviceEntry`], [`DiagnosticEntry`]) clients receive.
//!
//! The interface state is held behind an `Arc<Mutex<…>>` so the daemon's
//! background hot-plug thread (running [`crate::watch::run_loop`]) can
//! refresh the snapshot and emit signals while D-Bus method calls keep
//! reading.
//!
//! # Wire surface
//!
//! Bus name: `org.whatcable.Devices`
//! Object path: `/org/whatcable/Devices`
//! Interface: `org.whatcable.Devices1`
//!
//! | Member | Signature | Notes |
//! |---|---|---|
//! | `ListDevices` | `() → a(sssssasi)` | One [`DeviceEntry`] per summary. |
//! | `ListPorts` | `() → ai` | Type-C `port_number`s currently exposed. |
//! | `Diagnose` | `(i) → (ssssb)` | Returns `(present, bottleneck, summary, detail, is_warning)` for the given port; `present == false` and empty strings when there is no diagnostic to report. |
//! | `SnapshotJson` | `() → s` | Full structured snapshot serialised with `serde_json`. |
//! | `Refresh` | `() → u` | Force a re-enumeration; returns the new summary count. |
//! | `Version` (property) | `s` | The crate version string. |
//! | `DeviceCount` (property) | `u` | Number of summaries in the latest snapshot. |
//! | `DeviceAdded` (signal) | `(ss)` | `(id, headline)` for a newly attached device. |
//! | `DeviceRemoved` (signal) | `s` | `id` of a device that disappeared. |
//! | `CapabilityDegraded` (signal) | `(iss)` | `(port_number, summary, detail)` when a port's charging diagnostic newly raises `is_warning`. |
//! | `CapabilityRestored` (signal) | `i` | `port_number` whose previous warning has cleared. |

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::Type;

use crate::sysfs::manager::{DeviceManager, Snapshot, SnapshotDiff};
use crate::summary::DeviceSummary;

/// One device or Type-C port as published over D-Bus.
///
/// A serialisable mirror of [`DeviceSummary`] containing only the fields a
/// thin client actually needs to render. Use [`DevicesIface`] /
/// `SnapshotJson` for the full structured view.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceEntry {
    /// Stable identifier — see [`DeviceSummary::id`].
    pub id: String,
    /// Category label (`"UsbDevice"`, `"TypeCPort"`, `"Hub"`).
    pub category: String,
    /// Status label (`"Empty"`, `"Connected"`, `"Charging"`).
    pub status: String,
    /// Single-line title (product name or `"USB-C Port N"`).
    pub headline: String,
    /// Single-line subtitle (vendor + class).
    pub subtitle: String,
    /// Suggested freedesktop icon name.
    pub icon: String,
    /// Body lines (one per UI bullet).
    pub bullets: Vec<String>,
    /// Type-C port number when [`Self::category`] is `"TypeCPort"`,
    /// otherwise `-1`.
    pub port_number: i32,
}

impl From<&DeviceSummary> for DeviceEntry {
    fn from(s: &DeviceSummary) -> Self {
        let port_number = s
            .typec_port
            .as_ref()
            .map(|p| p.port_number)
            .unwrap_or(-1);
        DeviceEntry {
            id: s.id(),
            category: format!("{:?}", s.category),
            status: format!("{:?}", s.status),
            headline: s.headline.clone(),
            subtitle: s.subtitle.clone(),
            icon: s.icon.clone(),
            bullets: s.bullets.clone(),
            port_number,
        }
    }
}

/// Charging diagnostic returned by `Diagnose(port)`.
///
/// `present == false` indicates the port has no diagnostic to report (no
/// charger advertised, port not enumerated, …); the remaining string fields
/// are empty in that case.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct DiagnosticEntry {
    /// `true` when a diagnostic was computed for the requested port.
    pub present: bool,
    /// Bottleneck variant name (`"NoCharger"`, `"CableLimit"`, …).
    pub bottleneck: String,
    /// One-line headline (`"Cable is limiting charging speed"`).
    pub summary: String,
    /// Optional second-line detail.
    pub detail: String,
    /// `true` when the bottleneck is user-actionable (swap the cable).
    pub is_warning: bool,
}

/// Mutable state shared between the hot-plug loop and D-Bus dispatch.
///
/// Held behind an `Arc<Mutex<…>>` by the daemon. [`Self::refresh`] re-reads
/// `/sys`, computes the diff against the previous snapshot, and updates
/// `previous` so the next call can detect further deltas.
pub struct State {
    /// Underlying device manager that owns the live [`Snapshot`].
    pub manager: DeviceManager,
    /// Snapshot captured at the previous refresh — used as the diff base.
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

/// `org.whatcable.Devices1` interface implementation.
///
/// Hold one of these in an [`Arc`] (the inner `state` is already shared)
/// and register it with `connection.object_server().at(PATH, iface)`.
pub struct DevicesIface {
    /// Shared state backing every method call.
    pub state: Arc<Mutex<State>>,
}

impl DevicesIface {
    /// Helper for tests / clients embedding the iface in-process. Returns
    /// the same vector that the `ListDevices` D-Bus method would.
    pub fn snapshot_entries(&self) -> Vec<DeviceEntry> {
        let state = self.state.lock().expect("state mutex poisoned");
        state.manager.devices().iter().map(DeviceEntry::from).collect()
    }

    /// Helper for tests / clients embedding the iface in-process. Returns
    /// the same payload as the `Diagnose` D-Bus method.
    pub fn diagnose_port(&self, port_number: i32) -> DiagnosticEntry {
        let state = self.state.lock().expect("state mutex poisoned");
        let Some(summary) = state
            .manager
            .devices()
            .iter()
            .find(|s| s.typec_port.as_ref().is_some_and(|p| p.port_number == port_number))
        else {
            return DiagnosticEntry::default();
        };
        match &summary.charging_diag {
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

#[interface(name = "org.whatcable.Devices1")]
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
    ///
    /// Signals are *not* emitted from this path — they fire from the
    /// daemon's hot-plug thread. Calling `Refresh` simply makes the next
    /// `ListDevices` see the freshly read tree.
    fn refresh(&mut self) -> u32 {
        let mut state = self.state.lock().expect("state mutex poisoned");
        let _ = state.refresh();
        state.manager.devices().len() as u32
    }

    /// Crate version, e.g. `"0.4.0"`.
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

    /// Emitted when a port's charging diagnostic newly raises `is_warning`
    /// (e.g. a brand-new under-rated cable was plugged into a beefy charger).
    #[zbus(signal)]
    pub async fn capability_degraded(
        emitter: &SignalEmitter<'_>,
        port_number: i32,
        summary: String,
        detail: String,
    ) -> zbus::Result<()>;

    /// Emitted when a port that was previously raising `is_warning` no
    /// longer is — the bottleneck cleared (cable swapped, charger removed).
    #[zbus(signal)]
    pub async fn capability_restored(
        emitter: &SignalEmitter<'_>,
        port_number: i32,
    ) -> zbus::Result<()>;
}

/// Bus name the daemon requests on the session bus.
pub const BUS_NAME: &str = "org.whatcable.Devices";
/// Object path the [`DevicesIface`] is published at.
pub const OBJECT_PATH: &str = "/org/whatcable/Devices";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::Status;
    use crate::typec::{TypeCPartner, TypeCPort};
    use crate::usb::UsbDevice;

    #[test]
    fn diagnostic_entry_default_is_absent() {
        let d = DiagnosticEntry::default();
        assert!(!d.present);
        assert_eq!(d.summary, "");
    }

    #[test]
    fn device_entry_from_typec_summary_carries_port_number() {
        let port = TypeCPort {
            port_number: 7,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, None);
        let e = DeviceEntry::from(&s);
        assert_eq!(e.port_number, 7);
        assert_eq!(e.category, "TypeCPort");
        assert_eq!(e.status, format!("{:?}", Status::Connected));
        assert_eq!(e.id, "typec:");
        // ^ port_name is unset on this hand-built TypeCPort; the format is
        // still `typec:<port_name>`.
    }

    #[test]
    fn device_entry_from_usb_summary_marks_no_port_number() {
        let usb = UsbDevice {
            bus_port: "1-1".into(),
            product: "thing".into(),
            ..Default::default()
        };
        let s = DeviceSummary::from_usb_device(&usb);
        let e = DeviceEntry::from(&s);
        assert_eq!(e.port_number, -1);
        assert_eq!(e.id, "usb:1-1");
        assert_eq!(e.headline, "thing");
    }

    #[test]
    fn empty_state_yields_empty_iface_responses() {
        let manager = DeviceManager::with_sysfs(crate::Sysfs::with_root("/no/such/whatcable"));
        let state = Arc::new(Mutex::new(State::new(manager)));
        let iface = DevicesIface { state };
        assert!(iface.snapshot_entries().is_empty());
        let d = iface.diagnose_port(0);
        assert!(!d.present);
    }
}
