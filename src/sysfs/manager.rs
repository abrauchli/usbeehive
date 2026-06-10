//! Top-level snapshot: enumerate USB + Type-C + PD via [`Sysfs`], build
//! [`DeviceSummary`] aggregates.

use crate::cable::CableInfo;
use crate::power::PowerDeliveryPort;
use crate::summary::DeviceSummary;
use crate::thunderbolt::{self, ThunderboltRouter};
use crate::typec::TypeCPort;
use crate::usb::UsbDevice;

use super::reader::Sysfs;

/// Bundle of structured data captured by one [`DeviceManager::refresh`].
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// USB devices observed in `/sys/bus/usb/devices/`.
    pub usb_devices: Vec<UsbDevice>,
    /// Type-C ports observed in `/sys/class/typec/`.
    pub typec_ports: Vec<TypeCPort>,
    /// USB-PD ports observed in `/sys/class/usb_power_delivery/`.
    pub pd_ports: Vec<PowerDeliveryPort>,
    /// Thunderbolt / USB4 routers observed in `/sys/bus/thunderbolt/devices/`.
    pub thunderbolt_routers: Vec<ThunderboltRouter>,
    /// Plain-English summaries (one per non-root-hub device + one per Type-C port).
    pub summaries: Vec<DeviceSummary>,
}

/// What changed between two [`Snapshot`]s.
///
/// Computed by [`Snapshot::diff`]. Identifiers come from
/// [`DeviceSummary::id`] (`"typec:port0"`, `"usb:1-1.4"`, …). The
/// `*_degraded` and `*_resolved` lists carry Type-C port numbers — those
/// are the only summaries that can carry a [`crate::ChargingDiagnostic`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Summary ids that exist in `self` but not in the previous snapshot.
    pub added: Vec<String>,
    /// Summary ids that existed in the previous snapshot but are gone.
    pub removed: Vec<String>,
    /// Type-C port numbers whose charging diagnostic newly raised
    /// `is_warning` (or whose charger first appeared with a warning).
    pub newly_degraded: Vec<i32>,
    /// Type-C port numbers whose previously-warning diagnostic has cleared
    /// (e.g. user swapped the cable for a properly-rated one).
    pub resolved: Vec<i32>,
}

impl SnapshotDiff {
    /// `true` when no entries appeared, disappeared, or changed.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.newly_degraded.is_empty()
            && self.resolved.is_empty()
    }
}

impl Snapshot {
    /// Compute the difference of `self` relative to `previous`.
    ///
    /// "Added" / "removed" use [`DeviceSummary::id`]; "degraded" / "resolved"
    /// use Type-C `port_number`. Warnings carried by ports that already
    /// existed in `previous` are only re-reported when their `is_warning`
    /// flag flipped.
    pub fn diff(&self, previous: &Snapshot) -> SnapshotDiff {
        use std::collections::{HashMap, HashSet};

        let prev_ids: HashSet<String> = previous.summaries.iter().map(|s| s.id()).collect();
        let cur_ids: HashSet<String> = self.summaries.iter().map(|s| s.id()).collect();

        let added: Vec<String> = self
            .summaries
            .iter()
            .map(|s| s.id())
            .filter(|id| !prev_ids.contains(id))
            .collect();
        let removed: Vec<String> = previous
            .summaries
            .iter()
            .map(|s| s.id())
            .filter(|id| !cur_ids.contains(id))
            .collect();

        // Map port_number → was_warning for the previous snapshot.
        let mut prev_warn: HashMap<i32, bool> = HashMap::new();
        for s in &previous.summaries {
            if let Some(tc) = &s.typec_port {
                let is_warn = s.charging_diag.as_ref().is_some_and(|d| d.is_warning);
                prev_warn.insert(tc.port_number, is_warn);
            }
        }

        let mut newly_degraded = Vec::new();
        let mut resolved = Vec::new();
        for s in &self.summaries {
            let Some(tc) = &s.typec_port else {
                continue;
            };
            let now_warn = s.charging_diag.as_ref().is_some_and(|d| d.is_warning);
            match prev_warn.get(&tc.port_number).copied() {
                Some(true) if !now_warn => resolved.push(tc.port_number),
                Some(false) | None if now_warn => newly_degraded.push(tc.port_number),
                _ => {}
            }
        }

        SnapshotDiff {
            added,
            removed,
            newly_degraded,
            resolved,
        }
    }
}

/// Stateful enumerator that keeps the latest [`Snapshot`] in memory.
///
/// ```no_run
/// use usbeehive::DeviceManager;
///
/// let mut mgr = DeviceManager::new();
/// mgr.refresh();
/// for s in mgr.devices() {
///     println!("{}: {}", s.headline, s.subtitle);
/// }
/// ```
#[derive(Debug, Default)]
pub struct DeviceManager {
    sysfs: Sysfs,
    snapshot: Snapshot,
}

impl Default for Sysfs {
    fn default() -> Self {
        Sysfs::linux()
    }
}

impl DeviceManager {
    /// Build a manager bound to the standard Linux sysfs root.
    pub fn new() -> Self {
        Self::with_sysfs(Sysfs::linux())
    }

    /// Build a manager bound to `sysfs`. Useful for fixture-based tests
    /// or for inspecting captured trees.
    pub fn with_sysfs(sysfs: Sysfs) -> Self {
        DeviceManager {
            sysfs,
            snapshot: Snapshot::default(),
        }
    }

    /// Re-read every backing directory and rebuild [`Snapshot::summaries`].
    pub fn refresh(&mut self) {
        let usb_devices = self.sysfs.usb_devices();
        let typec_ports = self.sysfs.typec_ports();
        let pd_ports = self.sysfs.pd_ports();
        let thunderbolt_routers = self.sysfs.thunderbolt_routers();
        let summaries =
            build_summaries(&usb_devices, &typec_ports, &pd_ports, &thunderbolt_routers);
        self.snapshot = Snapshot {
            usb_devices,
            typec_ports,
            pd_ports,
            thunderbolt_routers,
            summaries,
        };
    }

    /// Latest snapshot.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Convenience accessor — same as `self.snapshot().summaries`.
    pub fn devices(&self) -> &[DeviceSummary] {
        &self.snapshot.summaries
    }

    /// Convenience accessor — USB devices.
    pub fn usb_devices(&self) -> &[UsbDevice] {
        &self.snapshot.usb_devices
    }

    /// Convenience accessor — Type-C ports.
    pub fn typec_ports(&self) -> &[TypeCPort] {
        &self.snapshot.typec_ports
    }

    /// Convenience accessor — USB-PD ports.
    pub fn pd_ports(&self) -> &[PowerDeliveryPort] {
        &self.snapshot.pd_ports
    }

    /// Convenience accessor — Thunderbolt / USB4 routers.
    pub fn thunderbolt_routers(&self) -> &[ThunderboltRouter] {
        &self.snapshot.thunderbolt_routers
    }

    /// The sysfs handle this manager was constructed with.
    pub fn sysfs(&self) -> &Sysfs {
        &self.sysfs
    }
}

/// Build per-device summaries from the structured snapshot inputs. Public so
/// callers using a non-sysfs backend can still get the aggregate view.
///
/// `tb` carries the Thunderbolt / USB4 router list — used to derive the
/// `transport.usb4` property on Type-C ports when both ends of the link
/// negotiated USB4. Pass `&[]` if you don't have the data.
pub fn build_summaries(
    usb: &[UsbDevice],
    ports: &[TypeCPort],
    pd: &[PowerDeliveryPort],
    tb: &[ThunderboltRouter],
) -> Vec<DeviceSummary> {
    let mut out = Vec::with_capacity(usb.len() + ports.len());
    let usb4_link = thunderbolt::system_has_usb4_link(tb);

    for tc in ports {
        // Pairing precedence:
        //   1. The partner's `usb_power_delivery` symlink — the kernel's
        //      canonical linkage; real `pdN` nodes carry no `parent_port*`
        //      attribute, so this is the only signal on live hardware.
        //   2. `parent_port_number` — the fixture convention; -1 ("not
        //      linked") never matches a real port number.
        //   3. Single-pd + single-port fallback.
        let pd_match = pd
            .iter()
            .find(|p| {
                tc.partner
                    .as_ref()
                    .is_some_and(|pa| !pa.pd_name.is_empty() && pa.pd_name == p.name)
            })
            .or_else(|| {
                pd.iter()
                    .find(|p| p.parent_port_number >= 0 && p.parent_port_number == tc.port_number)
            })
            .cloned()
            .or_else(|| {
                if pd.len() == 1 && ports.len() == 1 {
                    Some(pd[0].clone())
                } else {
                    None
                }
            });
        // Resolve the correlated USB device via the partner's USB child
        // directory (e.g. `2-2`) — the kernel's canonical port↔USB-device
        // linkage, same shape as the `usb_power_delivery` symlink pairing.
        // Chargers enumerate no USB device, so `usb_name` is empty there
        // and the lookup is a no-op.
        let usb_match = tc
            .partner
            .as_ref()
            .filter(|pa| !pa.usb_name.is_empty())
            .and_then(|pa| usb.iter().find(|d| d.bus_port == pa.usb_name));
        let cable = tc.cable.as_ref().map(CableInfo::from_typec_cable);
        let mut summary = DeviceSummary::from_typec_port(tc, pd_match, cable, usb_match);
        // USB4 isn't an altmode — it's negotiated in PD enter-mode and only
        // surfaces through the `thunderbolt` subsystem. Fire `transport.usb4`
        // when the system has an active USB4 link AND this port has a
        // partner that could plausibly be carrying it.
        if usb4_link && tc.partner.is_some() {
            summary
                .properties
                .push(("transport.usb4".into(), "true".into()));
        }
        out.push(summary);
    }

    for d in usb {
        if d.is_root_hub {
            continue;
        }
        out.push(DeviceSummary::from_usb_device(d));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::PowerDataObject;
    use crate::summary::Category;
    use crate::typec::TypeCPartner;

    #[test]
    fn root_hubs_are_excluded() {
        let root = UsbDevice {
            is_root_hub: true,
            bus_port: "usb1".into(),
            ..Default::default()
        };
        let child = UsbDevice {
            bus_port: "1-1".into(),
            product: "thing".into(),
            ..Default::default()
        };
        let summaries = build_summaries(&[root, child], &[], &[], &[]);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].headline, "thing");
        assert_eq!(summaries[0].category, Category::UsbDevice);
    }

    #[test]
    fn single_port_pd_pairs_with_single_typec() {
        let port = TypeCPort {
            port_number: 0,
            cable: None,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let pd = PowerDeliveryPort {
            parent_port_number: -1,
            source_capabilities: vec![PowerDataObject {
                power_mw: 60_000,
                ..Default::default()
            }],
            max_source_power_mw: 60_000,
            ..Default::default()
        };
        let summaries = build_summaries(&[], &[port], &[pd], &[]);
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].power_delivery.is_some());
    }

    #[test]
    fn multi_port_pd_matches_by_parent_number() {
        let p0 = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let p1 = TypeCPort {
            port_number: 1,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let pd_for_p1 = PowerDeliveryPort {
            parent_port_number: 1,
            source_capabilities: vec![PowerDataObject {
                power_mw: 100_000,
                ..Default::default()
            }],
            max_source_power_mw: 100_000,
            ..Default::default()
        };
        let summaries = build_summaries(&[], &[p0, p1], &[pd_for_p1], &[]);
        assert!(summaries[0].power_delivery.is_none());
        assert!(summaries[1].power_delivery.is_some());
    }

    #[test]
    fn partner_symlink_pairs_pd_and_unlinked_pd_never_matches_port0() {
        // Mirrors the real-laptop layout: two ports, charger on port1, and
        // the pd nodes carry no parent_port_number (-1). The partner's
        // pd_name is the only linkage. Regression: the derived Default
        // used to leave parent_port_number at 0, spuriously pairing every
        // unlinked pd node with port 0.
        let p0 = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let p1 = TypeCPort {
            port_number: 1,
            partner: Some(TypeCPartner {
                pd_name: "pd1".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pd0 = PowerDeliveryPort {
            name: "pd0".into(),
            parent_port_number: -1,
            source_capabilities: vec![PowerDataObject {
                power_mw: 45_000,
                ..Default::default()
            }],
            max_source_power_mw: 45_000,
            ..Default::default()
        };
        let pd1 = PowerDeliveryPort {
            name: "pd1".into(),
            parent_port_number: -1,
            source_capabilities: vec![PowerDataObject {
                power_mw: 65_000,
                ..Default::default()
            }],
            max_source_power_mw: 65_000,
            ..Default::default()
        };
        let summaries = build_summaries(&[], &[p0, p1], &[pd0, pd1], &[]);
        assert!(
            summaries[0].power_delivery.is_none(),
            "unlinked pd node must not pair with port 0"
        );
        let paired = summaries[1].power_delivery.as_ref().unwrap();
        assert_eq!(paired.name, "pd1");
        assert_eq!(paired.max_source_power_mw, 65_000);
    }

    #[test]
    fn partner_symlink_wins_over_parent_port_number() {
        // Symlink linkage is the kernel's canonical signal — it must beat
        // a (fixture-style) parent_port_number pointing elsewhere.
        let p0 = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner {
                pd_name: "pd1".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pd0 = PowerDeliveryPort {
            name: "pd0".into(),
            parent_port_number: 0,
            source_capabilities: vec![PowerDataObject {
                power_mw: 45_000,
                ..Default::default()
            }],
            max_source_power_mw: 45_000,
            ..Default::default()
        };
        let pd1 = PowerDeliveryPort {
            name: "pd1".into(),
            parent_port_number: -1,
            source_capabilities: vec![PowerDataObject {
                power_mw: 65_000,
                ..Default::default()
            }],
            max_source_power_mw: 65_000,
            ..Default::default()
        };
        let summaries = build_summaries(&[], &[p0], &[pd0, pd1], &[]);
        let paired = summaries[0].power_delivery.as_ref().unwrap();
        assert_eq!(paired.name, "pd1");
    }

    #[test]
    fn snapshot_diff_detects_added_and_removed_summaries() {
        let usb_a = UsbDevice {
            bus_port: "1-1".into(),
            product: "thing-a".into(),
            ..Default::default()
        };
        let usb_b = UsbDevice {
            bus_port: "1-2".into(),
            product: "thing-b".into(),
            ..Default::default()
        };
        let prev = Snapshot {
            summaries: build_summaries(std::slice::from_ref(&usb_a), &[], &[], &[]),
            ..Default::default()
        };
        let cur = Snapshot {
            summaries: build_summaries(std::slice::from_ref(&usb_b), &[], &[], &[]),
            ..Default::default()
        };
        let diff = cur.diff(&prev);
        assert_eq!(diff.added, vec!["usb:1-2"]);
        assert_eq!(diff.removed, vec!["usb:1-1"]);
        assert!(diff.newly_degraded.is_empty());
        assert!(diff.resolved.is_empty());
        assert!(!diff.is_empty());
    }

    #[test]
    fn snapshot_diff_flags_newly_degraded_typec_port() {
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let charger = PowerDeliveryPort {
            parent_port_number: 0,
            source_capabilities: vec![PowerDataObject {
                power_mw: 100_000,
                ..Default::default()
            }],
            max_source_power_mw: 100_000,
            ..Default::default()
        };

        // Previous: same port, no charger → no diagnostic.
        let prev = Snapshot {
            summaries: build_summaries(&[], std::slice::from_ref(&port), &[], &[]),
            ..Default::default()
        };
        // Current: charger plugged in, but the cable is rated only 60W.
        let mut summaries = build_summaries(
            &[],
            std::slice::from_ref(&port),
            std::slice::from_ref(&charger),
            &[],
        );
        // Inject a CableLimit warning so we don't depend on the cable decoder
        // for this assertion.
        summaries[0].charging_diag = Some(crate::diagnostic::ChargingDiagnostic {
            bottleneck: crate::diagnostic::Bottleneck::CableLimit,
            summary: "Cable is limiting charging speed".into(),
            detail: "Cable rated for 60W, but charger can deliver 100W".into(),
            is_warning: true,
        });
        let cur = Snapshot {
            summaries,
            ..Default::default()
        };
        let diff = cur.diff(&prev);
        assert_eq!(diff.newly_degraded, vec![0]);
        assert!(diff.resolved.is_empty());
    }

    #[test]
    fn snapshot_diff_flags_resolved_when_warning_clears() {
        let port = TypeCPort {
            port_number: 1,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let mut prev_summaries = build_summaries(&[], std::slice::from_ref(&port), &[], &[]);
        prev_summaries[0].charging_diag = Some(crate::diagnostic::ChargingDiagnostic {
            bottleneck: crate::diagnostic::Bottleneck::CableLimit,
            summary: "Cable is limiting charging speed".into(),
            detail: "".into(),
            is_warning: true,
        });
        let prev = Snapshot {
            summaries: prev_summaries,
            ..Default::default()
        };
        // Current: same port, no warning anymore.
        let cur = Snapshot {
            summaries: build_summaries(&[], std::slice::from_ref(&port), &[], &[]),
            ..Default::default()
        };
        let diff = cur.diff(&prev);
        assert_eq!(diff.resolved, vec![1]);
        assert!(diff.newly_degraded.is_empty());
    }

    #[test]
    fn snapshot_diff_empty_when_unchanged() {
        let usb = UsbDevice {
            bus_port: "1-1".into(),
            product: "thing".into(),
            ..Default::default()
        };
        let snap = Snapshot {
            summaries: build_summaries(&[usb], &[], &[], &[]),
            ..Default::default()
        };
        let diff = snap.diff(&snap.clone());
        assert!(diff.is_empty(), "{diff:?}");
    }

    #[test]
    fn manager_with_missing_sysfs_yields_empty_snapshot() {
        let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/usbeehive/root"));
        mgr.refresh();
        let s = mgr.snapshot();
        assert!(s.usb_devices.is_empty());
        assert!(s.typec_ports.is_empty());
        assert!(s.pd_ports.is_empty());
        assert!(s.thunderbolt_routers.is_empty());
        assert!(s.summaries.is_empty());
    }

    #[test]
    fn transport_usb4_fires_for_port_with_partner_when_usb4_link_present() {
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let routers = vec![
            ThunderboltRouter {
                route: "0-0".into(),
                is_host: true,
                generation: 4,
                ..Default::default()
            },
            ThunderboltRouter {
                route: "0-1".into(),
                is_host: false,
                generation: 4,
                ..Default::default()
            },
        ];
        let summaries = build_summaries(&[], std::slice::from_ref(&port), &[], &routers);
        let s = &summaries[0];
        assert!(s
            .properties
            .iter()
            .any(|(k, v)| k == "transport.usb4" && v == "true"));
    }

    #[test]
    fn transport_usb4_silent_when_only_tbt3_partner() {
        // USB4-capable host, but the partner is a TBT3-only dock — link
        // runs at TBT3, not USB4.
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
            ..Default::default()
        };
        let routers = vec![
            ThunderboltRouter {
                route: "0-0".into(),
                is_host: true,
                generation: 4,
                ..Default::default()
            },
            ThunderboltRouter {
                route: "0-1".into(),
                is_host: false,
                generation: 3,
                ..Default::default()
            },
        ];
        let summaries = build_summaries(&[], std::slice::from_ref(&port), &[], &routers);
        assert!(!summaries[0]
            .properties
            .iter()
            .any(|(k, _)| k == "transport.usb4"));
    }

    #[test]
    fn transport_usb4_silent_for_empty_port() {
        // No partner attached → don't ascribe USB4 to the port even if the
        // system has an active link elsewhere.
        let port = TypeCPort {
            port_number: 0,
            partner: None,
            cable: None,
            ..Default::default()
        };
        let routers = vec![
            ThunderboltRouter {
                route: "0-0".into(),
                is_host: true,
                generation: 4,
                ..Default::default()
            },
            ThunderboltRouter {
                route: "0-1".into(),
                is_host: false,
                generation: 4,
                ..Default::default()
            },
        ];
        let summaries = build_summaries(&[], std::slice::from_ref(&port), &[], &routers);
        assert!(!summaries[0]
            .properties
            .iter()
            .any(|(k, _)| k == "transport.usb4"));
    }

    #[test]
    fn usb_correlation_via_usb_name() {
        // Port with a partner carrying usb_name "2-2" + a matching USB device
        // → subtitle is the USB product string and usb_device property present.
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner {
                usb_name: "2-2".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let usb_dev = crate::usb::UsbDevice {
            bus_port: "2-2".into(),
            product: "Pixel 7".into(),
            ..Default::default()
        };
        let summaries = build_summaries(&[usb_dev], std::slice::from_ref(&port), &[], &[]);
        let s = &summaries[0];
        // Subtitle should be the USB product string.
        assert_eq!(s.subtitle, "Pixel 7", "expected USB product subtitle");
        // usb_device cross-ref property.
        assert!(
            s.properties
                .iter()
                .any(|(k, v)| k == "usb_device" && v == "usb:2-2"),
            "expected usb_device property"
        );
    }

    #[test]
    fn usb_correlation_noop_for_charger_partner() {
        // Charger port whose partner has empty usb_name → no usb_device property.
        let charger_port = TypeCPort {
            port_number: 1,
            partner: Some(TypeCPartner {
                usb_name: String::new(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let summaries = build_summaries(&[], std::slice::from_ref(&charger_port), &[], &[]);
        let s = &summaries[0];
        // usb_device property must be absent for a charger.
        assert!(
            !s.properties.iter().any(|(k, _)| k == "usb_device"),
            "charger must not get usb_device property"
        );
    }
}
