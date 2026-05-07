//! Text and JSON rendering for [`DeviceManager`].

use std::collections::HashSet;
use std::io::{self, Write};

use serde_json::{json, Map, Value};
use whatcable::pd::{cable_current_label, cable_speed_label};
use whatcable::summary::{Category, DeviceSummary};
use whatcable::usb::UsbDevice;
use whatcable::usbclass;
use whatcable::DeviceManager;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";
const BRIGHT_BLACK: &str = "\x1b[90m";
const BRIGHT_BLUE: &str = "\x1b[94m";
const BRIGHT_MAGENTA: &str = "\x1b[95m";

/// Render the manager's current snapshot as colorized text.
pub fn print_text<W: Write>(w: &mut W, mgr: &DeviceManager, show_raw: bool) -> io::Result<()> {
    print_text_iter(w, mgr.devices(), show_raw)
}

fn print_text_iter<W: Write>(
    w: &mut W,
    devices: &[DeviceSummary],
    show_raw: bool,
) -> io::Result<()> {
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

/// Render the bus topology as a colorized tree.
///
/// Color encodes the upstream link speed of each node (see `speed_color`).
/// Hubs render in italic. Type-C ports are listed above the tree.
pub fn print_tree<W: Write>(w: &mut W, mgr: &DeviceManager) -> io::Result<()> {
    let mut printed_anything = false;

    for s in mgr.devices() {
        if s.category == Category::TypeCPort {
            print_typec_summary(w, s)?;
            printed_anything = true;
        }
    }
    if printed_anything {
        writeln!(w)?;
    }

    let usb = mgr.usb_devices();
    let roots = collect_tree_roots(usb);
    if roots.is_empty() {
        if !printed_anything {
            return writeln!(w, "No USB devices found.");
        }
        return Ok(());
    }

    let mut last_was_root_block = false;
    for root in roots {
        if root.is_root_hub && root.children.is_empty() {
            continue;
        }
        if last_was_root_block {
            writeln!(w)?;
        }
        print_root(w, root)?;
        last_was_root_block = true;
    }

    writeln!(w)?;
    print_legend(w)
}

fn print_typec_summary<W: Write>(w: &mut W, dev: &DeviceSummary) -> io::Result<()> {
    writeln!(w, "{BOLD}{CYAN}{}{RESET}", dev.headline)?;
    if !dev.subtitle.is_empty() {
        writeln!(w, "  {}", dev.subtitle)?;
    }
    for b in &dev.bullets {
        writeln!(w, "  {DIM}• {RESET}{b}")?;
    }
    Ok(())
}

fn print_root<W: Write>(w: &mut W, dev: &UsbDevice) -> io::Result<()> {
    let color = speed_color(dev.speed);
    if dev.is_root_hub {
        writeln!(
            w,
            "{color}● {}{RESET}  {DIM}{}{RESET}",
            dev.bus_port,
            root_label(dev)
        )?;
    } else {
        let style = if dev.is_hub { ITALIC } else { "" };
        writeln!(w, "{color}{style}{}{RESET}", dev.display_name())?;
    }
    let n = dev.children.len();
    for (i, child) in dev.children.iter().enumerate() {
        print_branch(w, child, "", i + 1 == n, dev)?;
    }
    Ok(())
}

fn print_branch<W: Write>(
    w: &mut W,
    dev: &UsbDevice,
    prefix: &str,
    last: bool,
    parent: &UsbDevice,
) -> io::Result<()> {
    let connector = if last { "└─ " } else { "├─ " };
    let color = speed_color(dev.speed);
    let style = if dev.is_hub { ITALIC } else { "" };
    let name = dev.display_name();

    let mut hint = String::new();
    if !dev.is_hub && name == parent.display_name() {
        let cls = first_class_name(dev);
        if !cls.is_empty() {
            hint = format!("  {DIM}({cls}){RESET}");
        }
    }

    writeln!(
        w,
        "{prefix}{color}{connector}{style}{name}{RESET}{hint}"
    )?;

    let next_prefix = format!("{prefix}{DIM}{}{RESET}", if last { "   " } else { "│  " });
    let n = dev.children.len();
    for (i, child) in dev.children.iter().enumerate() {
        print_branch(w, child, &next_prefix, i + 1 == n, dev)?;
    }
    Ok(())
}

fn print_legend<W: Write>(w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "{DIM}Link:{RESET}  \
         {BRIGHT_MAGENTA}●{RESET} {DIM}40G{RESET}  \
         {MAGENTA}●{RESET} {DIM}20G{RESET}  \
         {BRIGHT_BLUE}●{RESET} {DIM}10G{RESET}  \
         {CYAN}●{RESET} {DIM}5G{RESET}  \
         {GREEN}●{RESET} {DIM}480M{RESET}  \
         {YELLOW}●{RESET} {DIM}12M{RESET}  \
         {BRIGHT_BLACK}●{RESET} {DIM}1.5M{RESET}   \
         {DIM}{ITALIC}italic{RESET}{DIM} = hub{RESET}"
    )
}

fn speed_color(mbps: u32) -> &'static str {
    // Speeds <12 Mbps and unknown both bucket to gray. Some Low-Speed devices
    // report `speed = "1.5"` which fails int parse and arrives here as 0.
    match mbps {
        s if s >= 40000 => BRIGHT_MAGENTA,
        s if s >= 20000 => MAGENTA,
        s if s >= 10000 => BRIGHT_BLUE,
        s if s >= 5000 => CYAN,
        s if s >= 480 => GREEN,
        s if s >= 12 => YELLOW,
        _ => BRIGHT_BLACK,
    }
}

fn root_label(d: &UsbDevice) -> &'static str {
    match d.speed {
        s if s >= 40000 => "USB4 root",
        s if s >= 20000 => "USB 3.2 root",
        s if s >= 10000 => "USB 3.1 root",
        s if s >= 5000 => "USB 3.0 root",
        s if s >= 480 => "USB 2.0 root",
        s if s >= 12 => "USB 1.1 root",
        _ => "USB root",
    }
}

fn first_class_name(dev: &UsbDevice) -> String {
    if dev.device_class != 0 && dev.device_class != 0xFF {
        return usbclass::class_name(dev.device_class);
    }
    for iface in &dev.interfaces {
        let name = usbclass::class_name(iface.class_code);
        if name == "Composite" || name.starts_with("0x") {
            continue;
        }
        return name;
    }
    String::new()
}

/// A "tree root" is a device with no parent in `devs` — either a kernel
/// root hub (`is_root_hub`) or an orphan whose parent isn't enumerated
/// (e.g. a fixture-only device with no `usbN` entry).
fn collect_tree_roots(devs: &[UsbDevice]) -> Vec<&UsbDevice> {
    let names: HashSet<&str> = devs.iter().map(|d| d.bus_port.as_str()).collect();
    devs.iter()
        .filter(|d| {
            if d.is_root_hub {
                return true;
            }
            !names.contains(parent_bus_port(&d.bus_port).as_str())
        })
        .collect()
}

fn parent_bus_port(bp: &str) -> String {
    if let Some((head, _)) = bp.rsplit_once('.') {
        return head.to_string();
    }
    if let Some((bus, _)) = bp.split_once('-') {
        return format!("usb{bus}");
    }
    String::new()
}

/// Render the manager's snapshot as pretty-printed JSON.
pub fn print_json<W: Write>(w: &mut W, mgr: &DeviceManager, show_raw: bool) -> io::Result<()> {
    let arr: Vec<Value> = mgr
        .devices()
        .iter()
        .map(|d| device_json(d, show_raw))
        .collect();
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

pub(crate) fn device_json(dev: &DeviceSummary, show_raw: bool) -> Value {
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
                    "class": usbclass::class_name(i.class_code),
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
            if let Some(mw) = psy.negotiated_power_mw() {
                ps.insert("negotiatedPowerMW".into(), Value::Number(mw.into()));
            }
            ps.insert("chargeType".into(), Value::String(psy.charge_type.clone()));
            ps.insert("usbType".into(), Value::String(psy.usb_type.clone()));
            t.as_object_mut()
                .unwrap()
                .insert("powerSupply".into(), Value::Object(ps));
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
            cab.as_object_mut().unwrap().insert(
                "current".into(),
                Value::String(cable_current_label(curr).into()),
            );
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
    use whatcable::summary::Status;
    use whatcable::usb::UsbDevice;

    #[test]
    fn empty_text_says_no_devices() {
        let mut buf = Vec::new();
        print_text_iter(&mut buf, &[], false).unwrap();
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
        let port = whatcable::typec::TypeCPort {
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

    #[test]
    fn speed_color_buckets() {
        assert_eq!(speed_color(0), BRIGHT_BLACK);
        assert_eq!(speed_color(1), BRIGHT_BLACK);
        assert_eq!(speed_color(12), YELLOW);
        assert_eq!(speed_color(480), GREEN);
        assert_eq!(speed_color(5_000), CYAN);
        assert_eq!(speed_color(10_000), BRIGHT_BLUE);
        assert_eq!(speed_color(20_000), MAGENTA);
        assert_eq!(speed_color(40_000), BRIGHT_MAGENTA);
    }

    #[test]
    fn root_label_picks_spec_for_speed() {
        let mut d = UsbDevice {
            speed: 480,
            ..Default::default()
        };
        assert_eq!(root_label(&d), "USB 2.0 root");
        d.speed = 10_000;
        assert_eq!(root_label(&d), "USB 3.1 root");
        d.speed = 40_000;
        assert_eq!(root_label(&d), "USB4 root");
    }

    #[test]
    fn parent_bus_port_resolves_levels() {
        assert_eq!(parent_bus_port("5-2.4.1"), "5-2.4");
        assert_eq!(parent_bus_port("1-1"), "usb1");
        assert_eq!(parent_bus_port("usb5"), "");
    }

    #[test]
    fn collect_tree_roots_includes_orphans_and_root_hubs() {
        let root = UsbDevice {
            bus_port: "usb1".into(),
            is_root_hub: true,
            ..Default::default()
        };
        let attached = UsbDevice {
            bus_port: "1-1".into(),
            ..Default::default()
        };
        let orphan = UsbDevice {
            bus_port: "9-9".into(),
            ..Default::default()
        };
        let devs = vec![root, attached, orphan];
        let roots = collect_tree_roots(&devs);
        let names: Vec<&str> = roots.iter().map(|d| d.bus_port.as_str()).collect();
        assert!(names.contains(&"usb1"));
        assert!(names.contains(&"9-9"));
        assert!(!names.contains(&"1-1"));
    }

    #[test]
    fn print_tree_legend_appears_when_devices_exist() {
        use whatcable::DeviceManager;
        use whatcable::Sysfs;
        let mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/whatcable/path"));
        let mut buf = Vec::new();
        print_tree(&mut buf, &mgr).unwrap();
        let out = String::from_utf8_lossy(&buf);
        assert!(out.contains("No USB devices found."));
    }

    #[test]
    fn text_output_renders_charger_profiles() {
        use whatcable::power::{PdoType, PowerDataObject, PowerDeliveryPort};
        use whatcable::typec::{TypeCPartner, TypeCPort};
        let port = TypeCPort {
            port_number: 0,
            partner: Some(TypeCPartner::default()),
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
        let mut buf = Vec::new();
        print_text_iter(&mut buf, &[s], false).unwrap();
        let out = String::from_utf8_lossy(&buf);
        assert!(out.contains("Charger profiles:"));
        assert!(out.contains("20.0V @ 5.00A"));
        assert!(out.contains("◀ active"));
    }
}
