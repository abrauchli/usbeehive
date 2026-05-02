//! Text and JSON rendering for `DeviceSummary` lists.

use std::io::{self, Write};

use serde_json::{json, Map, Value};

use crate::manager::DeviceManager;
use crate::pd::{cable_current_label, cable_speed_label};
use crate::summary::{Category, DeviceSummary};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";

pub fn print_text<W: Write>(w: &mut W, mgr: &DeviceManager, show_raw: bool) -> io::Result<()> {
    let devices = mgr.devices();
    if devices.is_empty() {
        return writeln!(w, "No USB devices found.");
    }
    for dev in devices {
        let color = match dev.category {
            Category::TypeCPort => CYAN,
            Category::Hub => BLUE,
            Category::UsbDevice => GREEN,
        };
        writeln!(w, "{BOLD}{color}{}{RESET}", dev.headline)?;
        if !dev.subtitle.is_empty() {
            writeln!(w, "  {}", dev.subtitle)?;
        }
        for b in &dev.bullets {
            writeln!(w, "  {DIM}• {RESET}{b}")?;
        }
        if let Some(diag) = &dev.charging_diag {
            if diag.is_warning {
                writeln!(w, "  {YELLOW}⚠ {}{RESET}", diag.summary)?;
            } else {
                writeln!(w, "  {GREEN}✓ {}{RESET}", diag.summary)?;
            }
            if !diag.detail.is_empty() {
                writeln!(w, "    {DIM}{}{RESET}", diag.detail)?;
            }
        }
        if let Some(pd) = &dev.power_delivery {
            if !pd.source_capabilities.is_empty() {
                writeln!(w, "  {BOLD}Charger profiles:{RESET}")?;
                for pdo in &pd.source_capabilities {
                    let marker = if pdo.is_active {
                        format!("{GREEN} ◀ active{RESET}")
                    } else {
                        String::new()
                    };
                    writeln!(
                        w,
                        "    {} @ {} — {}{}",
                        pdo.voltage_label(),
                        pdo.current_label(),
                        pdo.power_label(),
                        marker
                    )?;
                }
            }
        }
        if show_raw {
            let attrs = dev
                .usb_device
                .as_ref()
                .map(|u| &u.raw_attributes)
                .or_else(|| dev.typec_port.as_ref().map(|t| &t.raw_attributes));
            if let Some(attrs) = attrs {
                if !attrs.is_empty() {
                    writeln!(w, "  {DIM}Raw sysfs attributes:{RESET}")?;
                    for (k, v) in attrs.iter() {
                        writeln!(w, "    {k} = {v}")?;
                    }
                }
            }
        }
        writeln!(w)?;
    }
    Ok(())
}

pub fn print_json<W: Write>(w: &mut W, mgr: &DeviceManager, show_raw: bool) -> io::Result<()> {
    let arr: Vec<Value> = mgr.devices().iter().map(|d| device_json(d, show_raw)).collect();
    let s = serde_json::to_string_pretty(&arr).unwrap();
    writeln!(w, "{s}")
}

fn hex_vidpid(v: u16) -> String {
    format!("0x{v:04x}")
}

fn raw_to_value(map: &std::collections::BTreeMap<String, String>) -> Value {
    let mut out = Map::new();
    for (k, v) in map {
        out.insert(k.clone(), Value::String(v.clone()));
    }
    Value::Object(out)
}

fn device_json(dev: &DeviceSummary, show_raw: bool) -> Value {
    let category = match dev.category {
        Category::TypeCPort => "typec",
        Category::Hub => "hub",
        Category::UsbDevice => "usb",
    };
    let mut obj = Map::new();
    obj.insert("category".into(), Value::String(category.into()));
    obj.insert("headline".into(), Value::String(dev.headline.clone()));
    obj.insert("subtitle".into(), Value::String(dev.subtitle.clone()));
    obj.insert("icon".into(), Value::String(dev.icon.clone()));
    obj.insert(
        "bullets".into(),
        Value::Array(dev.bullets.iter().cloned().map(Value::String).collect()),
    );

    if let Some(u) = &dev.usb_device {
        let interfaces: Vec<Value> = u
            .interfaces
            .iter()
            .map(|i| {
                json!({
                    "class": crate::usbclass::class_name(i.class_code),
                    "driver": i.driver,
                })
            })
            .collect();
        let mut usb = json!({
            "vendorId": hex_vidpid(u.vendor_id),
            "productId": hex_vidpid(u.product_id),
            "manufacturer": u.manufacturer,
            "product": u.product,
            "speed": u.speed,
            "speedLabel": u.speed_label(),
            "version": u.version,
            "maxPowerMA": u.max_power_ma,
            "serial": u.serial,
            "removable": u.removable,
            "bus": u.bus_num,
            "device": u.dev_num,
            "isHub": u.is_hub,
            "interfaces": interfaces,
        });
        if show_raw {
            usb.as_object_mut()
                .unwrap()
                .insert("raw".into(), raw_to_value(&u.raw_attributes));
        }
        obj.insert("usb".into(), usb);
    }

    if let Some(tc) = &dev.typec_port {
        let mut t = json!({
            "port": tc.port_number,
            "dataRole": tc.current_data_role(),
            "powerRole": tc.current_power_role(),
            "portType": tc.port_type,
            "powerOpMode": tc.power_op_mode,
            "connected": tc.is_connected(),
        });
        if let Some(psy) = &tc.power_supply {
            let mut ps = Map::new();
            ps.insert("name".into(), Value::String(psy.name.clone()));
            ps.insert("online".into(), Value::Bool(psy.online));
            for (k, v) in [
                ("voltageNowUV", psy.voltage_now_uv),
                ("currentNowUA", psy.current_now_ua),
                ("currentMaxUA", psy.current_max_ua),
                ("voltageMinUV", psy.voltage_min_uv),
                ("voltageMaxUV", psy.voltage_max_uv),
            ] {
                if let Some(val) = v {
                    ps.insert(k.into(), Value::Number(val.into()));
                }
            }
            if let (Some(v_uv), Some(i_ua)) = (psy.voltage_now_uv, psy.current_now_ua) {
                let mw = (v_uv as i128 * i_ua as i128 / 1_000_000_000) as i64;
                ps.insert("negotiatedPowerMW".into(), Value::Number(mw.into()));
            }
            ps.insert("chargeType".into(), Value::String(psy.charge_type.clone()));
            ps.insert("usbType".into(), Value::String(psy.usb_type.clone()));
            t.as_object_mut().unwrap().insert("powerSupply".into(), Value::Object(ps));
        }
        if show_raw {
            t.as_object_mut()
                .unwrap()
                .insert("raw".into(), raw_to_value(&tc.raw_attributes));
        }
        obj.insert("typec".into(), t);
    }

    if let Some(c) = &dev.cable {
        let mut cab = json!({
            "type": c.cable_type,
            "maxWatts": c.max_watts,
            "vendorId": hex_vidpid(c.vendor_id),
            "vendorName": c.vendor_name,
        });
        if let Some(s) = c.speed {
            cab.as_object_mut()
                .unwrap()
                .insert("speed".into(), Value::String(cable_speed_label(s).into()));
        }
        if let Some(curr) = c.current_rating {
            cab.as_object_mut()
                .unwrap()
                .insert("current".into(), Value::String(cable_current_label(curr).into()));
        }
        obj.insert("cable".into(), cab);
    }

    if let Some(pd) = &dev.power_delivery {
        let pdos: Vec<Value> = pd
            .source_capabilities
            .iter()
            .map(|p| {
                json!({
                    "type": p.r#type.label(),
                    "voltageMV": p.voltage_mv,
                    "currentMA": p.current_ma,
                    "powerMW": p.power_mw,
                    "active": p.is_active,
                })
            })
            .collect();
        obj.insert(
            "powerDelivery".into(),
            json!({
                "sourceCapabilities": pdos,
                "maxPowerMW": pd.max_source_power_mw,
            }),
        );
    }

    if let Some(d) = &dev.charging_diag {
        obj.insert(
            "charging".into(),
            json!({
                "summary": d.summary,
                "detail": d.detail,
                "isWarning": d.is_warning,
            }),
        );
    }

    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::Status;
    use crate::usb::UsbDevice;

    fn fake_mgr() -> DeviceManager {
        // Construct a manager with a hand-built summary by going through refresh
        // logic indirectly: we can't, so just exercise the JSON of a single
        // summary directly.
        DeviceManager::new()
    }

    #[test]
    fn empty_text_says_no_devices() {
        let mgr = fake_mgr();
        let mut buf = Vec::new();
        print_text(&mut buf, &mgr, false).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("No USB devices found."));
    }

    #[test]
    fn json_for_usb_summary_has_expected_keys() {
        let dev = UsbDevice {
            vendor_id: 0x05AC,
            product_id: 0x12A8,
            product: "iPhone".into(),
            version: "2.10".into(),
            speed: 480,
            ..Default::default()
        };
        let summary = DeviceSummary::from_usb_device(&dev);
        let v = device_json(&summary, false);
        let obj = v.as_object().unwrap();
        assert_eq!(obj["category"], "usb");
        assert_eq!(obj["headline"], "iPhone");
        let usb = obj["usb"].as_object().unwrap();
        assert_eq!(usb["vendorId"], "0x05ac");
        assert_eq!(usb["productId"], "0x12a8");
        assert_eq!(usb["speedLabel"], "High Speed 480 Mbps");
    }

    #[test]
    fn typec_summary_serializes() {
        let port = crate::typec::TypeCPort {
            port_number: 0,
            data_role: "host [device]".into(),
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, None);
        assert_eq!(s.status, Status::Empty);
        let v = device_json(&s, false);
        assert_eq!(v["category"], "typec");
        assert_eq!(v["typec"]["dataRole"], "device");
    }
}
