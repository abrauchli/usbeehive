//! Top-level snapshot: enumerate USB + Type-C + PD via [`Sysfs`], build
//! [`DeviceSummary`] aggregates.

use whatcable_core::cable::CableInfo;
use whatcable_core::power::PowerDeliveryPort;
use whatcable_core::summary::DeviceSummary;
use whatcable_core::typec::TypeCPort;
use whatcable_core::usb::UsbDevice;

use crate::sysfs::Sysfs;

/// Bundle of structured data captured by one [`DeviceManager::refresh`].
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// USB devices observed in `/sys/bus/usb/devices/`.
    pub usb_devices: Vec<UsbDevice>,
    /// Type-C ports observed in `/sys/class/typec/`.
    pub typec_ports: Vec<TypeCPort>,
    /// USB-PD ports observed in `/sys/class/usb_power_delivery/`.
    pub pd_ports: Vec<PowerDeliveryPort>,
    /// Plain-English summaries (one per non-root-hub device + one per Type-C port).
    pub summaries: Vec<DeviceSummary>,
}

/// Stateful enumerator that keeps the latest [`Snapshot`] in memory.
///
/// ```no_run
/// use whatcable_sysfs::DeviceManager;
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
        let summaries = build_summaries(&usb_devices, &typec_ports, &pd_ports);
        self.snapshot = Snapshot {
            usb_devices,
            typec_ports,
            pd_ports,
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

    /// The sysfs handle this manager was constructed with.
    pub fn sysfs(&self) -> &Sysfs {
        &self.sysfs
    }
}

/// Build per-device summaries from a structured triple. Public so callers
/// using a non-sysfs backend can still get the aggregate view.
pub fn build_summaries(
    usb: &[UsbDevice],
    ports: &[TypeCPort],
    pd: &[PowerDeliveryPort],
) -> Vec<DeviceSummary> {
    let mut out = Vec::with_capacity(usb.len() + ports.len());

    for tc in ports {
        let pd_match = pd
            .iter()
            .find(|p| p.parent_port_number == tc.port_number)
            .cloned()
            .or_else(|| {
                if pd.len() == 1 && ports.len() == 1 {
                    Some(pd[0].clone())
                } else {
                    None
                }
            });
        let cable = tc.cable.as_ref().map(CableInfo::from_typec_cable);
        out.push(DeviceSummary::from_typec_port(tc, pd_match, cable));
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
    use whatcable_core::power::PowerDataObject;
    use whatcable_core::summary::Category;
    use whatcable_core::typec::TypeCPartner;

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
        let summaries = build_summaries(&[root, child], &[], &[]);
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
        let summaries = build_summaries(&[], &[port], &[pd]);
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
        let summaries = build_summaries(&[], &[p0, p1], &[pd_for_p1]);
        assert!(summaries[0].power_delivery.is_none());
        assert!(summaries[1].power_delivery.is_some());
    }

    #[test]
    fn manager_with_missing_sysfs_yields_empty_snapshot() {
        let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/whatcable/root"));
        mgr.refresh();
        let s = mgr.snapshot();
        assert!(s.usb_devices.is_empty());
        assert!(s.typec_ports.is_empty());
        assert!(s.pd_ports.is_empty());
        assert!(s.summaries.is_empty());
    }
}
