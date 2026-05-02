//! Plain-English summary of a USB device or Type-C port.
//!
//! [`DeviceSummary`] is a CLI-friendly view: a headline, subtitle, bullet
//! list, and an icon hint, plus the raw structured data it was derived from.
//! It is the high-level façade most consumers will reach for.

use serde::Serialize;

use crate::cable::CableInfo;
use crate::diagnostic::ChargingDiagnostic;
use crate::pd::{decode_id_header, product_type_label};
use crate::power::PowerDeliveryPort;
use crate::typec::{TypeCPort, TypeCPowerSupply};
use crate::usb::UsbDevice;
use crate::usbclass;
use crate::vendor;

/// High-level grouping a [`DeviceSummary`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    /// Plain USB peripheral.
    UsbDevice,
    /// USB Type-C port (with or without a partner attached).
    TypeCPort,
    /// USB hub (anything with `device_class == 0x09`).
    Hub,
}

/// Connection status surfaced to the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Status {
    /// Type-C port with nothing attached.
    Empty,
    /// Connected (no PD source advertised).
    Connected,
    /// Connected and currently being charged from a PD source.
    Charging,
}

/// One renderable summary entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceSummary {
    /// High-level grouping for color/iconography.
    pub category: Category,
    /// Connection state.
    pub status: Status,
    /// Single-line title (e.g. `"USB-C Port 0"` or the product string).
    pub headline: String,
    /// Single-line subtitle (vendor + class).
    pub subtitle: String,
    /// Body lines.
    pub bullets: Vec<String>,
    /// Suggested freedesktop icon name for an icon theme.
    pub icon: String,

    /// Original USB device, if this summary describes one.
    pub usb_device: Option<UsbDevice>,
    /// Original Type-C port, if this summary describes one.
    pub typec_port: Option<TypeCPort>,
    /// Companion PD port for `typec_port`.
    pub power_delivery: Option<PowerDeliveryPort>,
    /// Decoded cable info attached to `typec_port`.
    pub cable: Option<CableInfo>,
    /// Computed charging diagnostic.
    pub charging_diag: Option<ChargingDiagnostic>,
}

fn power_contract_label(psy: &TypeCPowerSupply) -> Option<String> {
    let v_uv = psy.voltage_now_uv?;
    let i_ua = psy.current_now_ua?;
    if v_uv <= 0 || i_ua <= 0 {
        return None;
    }
    let volts = v_uv as f64 / 1_000_000.0;
    let amps = i_ua as f64 / 1_000_000.0;
    let watts = ((v_uv as i128 * i_ua as i128 + 500_000_000_000) / 1_000_000_000_000) as i64;
    Some(format!(
        "Negotiated power: {volts:.1}V @ {amps:.2}A — {watts}W"
    ))
}

fn icon_for(device_type: &str, is_hub: bool) -> &'static str {
    if is_hub {
        return "network-wired";
    }
    if device_type.contains("Audio") {
        "audio-card"
    } else if device_type.contains("HID") {
        "input-keyboard"
    } else if device_type.contains("Mass Storage") {
        "drive-removable-media"
    } else if device_type.contains("Video") {
        "camera-web"
    } else if device_type.contains("Wireless") {
        "network-wireless"
    } else if device_type.contains("Printer") {
        "printer"
    } else {
        "drive-removable-media-usb"
    }
}

impl DeviceSummary {
    /// Build a summary for a [`UsbDevice`].
    pub fn from_usb_device(dev: &UsbDevice) -> DeviceSummary {
        let vendor_name = vendor::lookup(dev.vendor_id);
        let has_vendor = !vendor::is_hex_fallback(&vendor_name);

        let device_type = if dev.device_class != 0 && dev.device_class != 0xFF {
            usbclass::class_name(dev.device_class)
        } else {
            let mut types: Vec<String> = Vec::new();
            for iface in &dev.interfaces {
                let t = usbclass::class_name(iface.class_code);
                if t == "Composite" || vendor::is_hex_fallback(&t) {
                    continue;
                }
                if !types.contains(&t) {
                    types.push(t);
                }
            }
            types.join(", ")
        };

        let mut subtitle = String::new();
        if has_vendor {
            subtitle.push_str(&vendor_name);
        }
        if !device_type.is_empty() {
            if !subtitle.is_empty() {
                subtitle.push_str(" · ");
            }
            subtitle.push_str(&device_type);
        }

        let mut bullets = vec![dev.speed_label().to_string()];
        if let Some(p) = dev.power_label() {
            bullets.push(format!("Power: {p}"));
        }
        bullets.push(format!("USB {}", dev.version));
        if !dev.serial.is_empty() {
            bullets.push(format!("Serial: {}", dev.serial));
        }
        match dev.removable.as_str() {
            "removable" => bullets.push("Removable".into()),
            "fixed" => bullets.push("Built-in".into()),
            _ => {}
        }
        let mut drivers: Vec<String> = Vec::new();
        for iface in &dev.interfaces {
            if !iface.driver.is_empty() && !drivers.contains(&iface.driver) {
                drivers.push(iface.driver.clone());
            }
        }
        if !drivers.is_empty() {
            bullets.push(format!("Drivers: {}", drivers.join(", ")));
        }
        bullets.push(format!(
            "VID:PID {:04x}:{:04x}",
            dev.vendor_id, dev.product_id
        ));

        DeviceSummary {
            category: if dev.is_hub { Category::Hub } else { Category::UsbDevice },
            status: Status::Connected,
            headline: dev.display_name(),
            subtitle,
            bullets,
            icon: icon_for(&device_type, dev.is_hub).to_string(),
            usb_device: Some(dev.clone()),
            typec_port: None,
            power_delivery: None,
            cable: None,
            charging_diag: None,
        }
    }

    /// Build a summary for a [`TypeCPort`], optionally enriched with the
    /// companion PD port and cable view.
    pub fn from_typec_port(
        port: &TypeCPort,
        pd: Option<PowerDeliveryPort>,
        cable_info: Option<CableInfo>,
    ) -> DeviceSummary {
        let mut s = DeviceSummary {
            category: Category::TypeCPort,
            status: Status::Empty,
            headline: format!("USB-C Port {}", port.port_number),
            subtitle: String::new(),
            bullets: Vec::new(),
            icon: "plug".into(),
            usb_device: None,
            typec_port: Some(port.clone()),
            power_delivery: pd,
            cable: cable_info,
            charging_diag: None,
        };

        if !port.is_connected() {
            s.subtitle = "Nothing connected".into();
            return s;
        }
        s.status = Status::Connected;

        if let Some(partner) = &port.partner {
            s.subtitle = match partner.identity.as_ref().and_then(|id| id.vdos.first()) {
                Some(&vdo) => {
                    let hdr = decode_id_header(vdo);
                    let product_label = product_type_label(
                        hdr.ufp_product_type.unwrap_or(crate::pd::ProductType::Undefined),
                    );
                    let vendor_label = vendor::lookup(hdr.vendor_id);
                    if vendor::is_hex_fallback(&vendor_label) {
                        product_label.to_string()
                    } else {
                        format!("{vendor_label} — {product_label}")
                    }
                }
                None => "Device connected".into(),
            };
        }

        let data = port.current_data_role();
        let power = port.current_power_role();
        if !data.is_empty() || !power.is_empty() {
            let mut role = String::new();
            if !data.is_empty() {
                role.push_str(&format!("Data: {data}"));
            }
            if !power.is_empty() {
                if !role.is_empty() {
                    role.push_str(", ");
                }
                role.push_str(&format!("Power: {power}"));
            }
            s.bullets.push(role);
        }

        if !port.power_op_mode.is_empty() {
            s.bullets.push(format!("Power mode: {}", port.power_op_mode));
        }

        if let Some(psy) = &port.power_supply {
            if psy.online {
                if let Some(c) = power_contract_label(psy) {
                    s.bullets.push(c);
                }
            }
        }

        if !port.pd_revision.is_empty() {
            s.bullets.push(format!("PD revision: {}", port.pd_revision));
        }
        if !port.orientation.is_empty() && port.orientation != "unknown" {
            s.bullets.push(format!("Plug orientation: {}", port.orientation));
        }

        if let Some(c) = &s.cable {
            if let Some(speed) = c.speed {
                s.bullets
                    .push(format!("Cable speed: {}", crate::pd::cable_speed_label(speed)));
            }
            if let Some(curr) = c.current_rating {
                s.bullets
                    .push(format!("Cable current: {}", crate::pd::cable_current_label(curr)));
            }
            if c.max_watts > 0 {
                s.bullets.push(format!("Cable max power: {}W", c.max_watts));
            }
            if c.is_active {
                s.bullets.push("Active cable".into());
            } else if c.is_passive {
                s.bullets.push("Passive cable".into());
            }
            if !c.vendor_name.is_empty() && !vendor::is_hex_fallback(&c.vendor_name) {
                s.bullets.push(format!("Cable vendor: {}", c.vendor_name));
            }
        }

        if let Some(pd_port) = &s.power_delivery {
            if !pd_port.source_capabilities.is_empty() {
                let max_w = pd_port.max_source_power_mw / 1000;
                s.bullets.push(format!("Charger max: {max_w}W"));
                s.status = Status::Charging;
            }
            s.charging_diag = ChargingDiagnostic::evaluate(pd_port, s.cable.as_ref());
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::UsbInterface;

    fn dev() -> UsbDevice {
        UsbDevice {
            vendor_id: 0x05AC,
            product_id: 0x12A8,
            product: "iPhone".into(),
            version: "2.10".into(),
            speed: 480,
            max_power_ma: 500,
            device_class: 0,
            interfaces: vec![UsbInterface {
                class_code: 0x03,
                driver: "usbhid".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn usb_summary_uses_vendor_and_class() {
        let s = DeviceSummary::from_usb_device(&dev());
        assert_eq!(s.headline, "iPhone");
        assert!(s.subtitle.starts_with("Apple"));
        assert!(s.subtitle.contains("HID"));
        assert!(s.bullets.iter().any(|b| b == "USB 2.10"));
        assert!(s.bullets.iter().any(|b| b.starts_with("VID:PID 05ac:")));
        assert!(s.bullets.iter().any(|b| b == "Drivers: usbhid"));
        assert_eq!(s.icon, "input-keyboard");
        assert_eq!(s.category, Category::UsbDevice);
    }

    #[test]
    fn hub_class_marks_hub() {
        let mut d = dev();
        d.device_class = 0x09;
        d.is_hub = true;
        let s = DeviceSummary::from_usb_device(&d);
        assert_eq!(s.category, Category::Hub);
        assert_eq!(s.icon, "network-wired");
    }

    #[test]
    fn empty_typec_port_is_disconnected() {
        let port = TypeCPort {
            port_number: 1,
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, None);
        assert_eq!(s.headline, "USB-C Port 1");
        assert_eq!(s.subtitle, "Nothing connected");
        assert_eq!(s.status, Status::Empty);
    }

    #[test]
    fn charging_status_when_pd_source_present() {
        use crate::power::{PdoType, PowerDataObject, PowerDeliveryPort};
        let port = TypeCPort {
            port_number: 0,
            partner: Some(crate::typec::TypeCPartner::default()),
            ..Default::default()
        };
        let pd = PowerDeliveryPort {
            source_capabilities: vec![PowerDataObject {
                r#type: PdoType::FixedSupply,
                voltage_mv: 20_000,
                current_ma: 5_000,
                power_mw: 100_000,
                is_active: true,
                ..Default::default()
            }],
            max_source_power_mw: 100_000,
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, Some(pd), None);
        assert_eq!(s.status, Status::Charging);
        assert!(s.bullets.iter().any(|b| b.contains("Charger max: 100W")));
        assert!(s.charging_diag.is_some());
    }

    #[test]
    fn power_contract_rounds_watts() {
        let psy = TypeCPowerSupply {
            online: true,
            voltage_now_uv: Some(9_000_000),
            current_now_ua: Some(2_000_000),
            ..Default::default()
        };
        let label = power_contract_label(&psy).unwrap();
        assert!(label.contains("9.0V"));
        assert!(label.contains("2.00A"));
        assert!(label.ends_with("18W"));
    }

    #[test]
    fn cable_bullets_appear() {
        use crate::pd::{CableCurrent, CableSpeed};
        let port = TypeCPort {
            port_number: 0,
            partner: Some(crate::typec::TypeCPartner::default()),
            ..Default::default()
        };
        let cable = CableInfo {
            cable_type: "passive".into(),
            speed: Some(CableSpeed::Usb32Gen2),
            current_rating: Some(CableCurrent::FiveAmp),
            max_watts: 100,
            is_passive: true,
            vendor_name: "Apple".into(),
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, Some(cable));
        assert!(s.bullets.iter().any(|b| b.contains("USB 3.2 Gen 2")));
        assert!(s.bullets.iter().any(|b| b.contains("Cable current: 5A")));
        assert!(s.bullets.iter().any(|b| b == "Cable max power: 100W"));
        assert!(s.bullets.iter().any(|b| b == "Passive cable"));
        assert!(s.bullets.iter().any(|b| b == "Cable vendor: Apple"));
    }
}
