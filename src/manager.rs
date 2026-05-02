//! Top-level snapshot: enumerate USB + Type-C + PD, build summaries.

use crate::cable::CableInfo;
use crate::power::PowerDeliveryPort;
use crate::summary::DeviceSummary;
use crate::typec::TypeCPort;
use crate::usb::UsbDevice;

#[derive(Default)]
pub struct DeviceManager {
    devices: Vec<DeviceSummary>,
    usb_devices: Vec<UsbDevice>,
    typec_ports: Vec<TypeCPort>,
    pd_ports: Vec<PowerDeliveryPort>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn devices(&self) -> &[DeviceSummary] {
        &self.devices
    }

    pub fn usb_devices(&self) -> &[UsbDevice] {
        &self.usb_devices
    }

    pub fn typec_ports(&self) -> &[TypeCPort] {
        &self.typec_ports
    }

    pub fn pd_ports(&self) -> &[PowerDeliveryPort] {
        &self.pd_ports
    }

    pub fn refresh(&mut self) {
        self.usb_devices = UsbDevice::enumerate();
        self.typec_ports = TypeCPort::enumerate();
        self.pd_ports = PowerDeliveryPort::enumerate();
        self.devices = build_summaries(&self.usb_devices, &self.typec_ports, &self.pd_ports);
    }
}

fn build_summaries(
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
                // Single-port systems often don't link the PD device to a typec
                // port number; if there's exactly one of each, pair them up.
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
    use crate::summary::Category;

    #[test]
    fn root_hubs_are_excluded() {
        let mut root = UsbDevice::default();
        root.is_root_hub = true;
        root.bus_port = "usb1".into();
        let mut child = UsbDevice::default();
        child.bus_port = "1-1".into();
        child.product = "thing".into();
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
            partner: Some(crate::typec::TypeCPartner::default()),
            ..Default::default()
        };
        let pd = PowerDeliveryPort {
            parent_port_number: -1,
            source_capabilities: vec![crate::power::PowerDataObject {
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
}
