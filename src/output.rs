//! Text and JSON rendering for [`DeviceManager`].

use std::io::{self, Write};

use serde_json::{json, Map, Value};
use usbeehive::pd::{cable_current_label, cable_speed_label};
use usbeehive::summary::{Category, DeviceSummary, PowerSummary};
use usbeehive::usb::{tree_roots, UsbDevice};
use usbeehive::usbclass;
use usbeehive::{DeviceManager, LinkSpeed};

/// Machine property keys → English display labels. The daemon emits
/// machine keys only; the CLI text renderer owns the display vocabulary.
/// Unknown keys fall through to their raw form so adding a new property
/// key on the daemon side surfaces without an `output.rs` change (just
/// uglier).
fn property_label(key: &str) -> String {
    match key {
        "serial" => "Serial".into(),
        "mount" => "Mount".into(),
        "drivers" => "Drivers".into(),
        "data_role" => "Data role".into(),
        "power_mode" => "Power mode".into(),
        "pd_revision" => "PD revision".into(),
        "plug_orientation" => "Plug orientation".into(),
        "pd_contract" => "PD contract".into(),
        "cable_speed" => "Cable speed".into(),
        "cable_max_current" => "Cable max current".into(),
        "cable_max_power" => "Cable max power".into(),
        "cable_type" => "Cable type".into(),
        "cable_vendor" => "Cable vendor".into(),
        "charger_max" => "Charger max".into(),
        "usb_max_power_ma" => "Max bus power (mA)".into(),
        "usb_device" => "USB device".into(),
        other => other.into(),
    }
}

/// Display label for boolean-flag properties (the `transport.*` family
/// and the `cable.trust.*` family), or `None` for value-bearing keys.
///
/// Flag keys are only present in the bag when their condition fires —
/// absence means "off", presence means "on" — so the renderer prints
/// just the label, not `key: true`.
fn property_flag_label(key: &str) -> Option<&'static str> {
    Some(match key {
        "transport.usb2" => "USB 2.0 link",
        "transport.usb3" => "USB 3.x link",
        "transport.usb4" => "USB4 link",
        "transport.dp_altmode" => "DisplayPort altmode",
        "transport.tb" => "Thunderbolt 3 altmode",
        "cable.trust.zero_vid" => "Cable trust: zero VID (likely counterfeit)",
        "cable.trust.vid_unknown" => "Cable trust: unknown VID",
        "cable.trust.reserved_bits" => "Cable trust: reserved bits set",
        // A hint, not a trust warning — renders in the normal style.
        "cable.no_emarker" => "No cable e-marker visible (3A limit may apply)",
        _ => return None,
    })
}

/// Render one property as a bullet line, handling both flag-style
/// (`{label}` when value == "true") and value-bearing (`{label}: {value}`).
fn write_property<W: Write>(w: &mut W, key: &str, value: &str) -> io::Result<()> {
    if let Some(label) = property_flag_label(key) {
        if value == "true" {
            // Cable-trust flags are warnings; colour them yellow so they
            // stand out against the routine transport / link bullets.
            let color = if key.starts_with("cable.trust.") {
                YELLOW
            } else {
                ""
            };
            return writeln!(w, "  {DIM}• {RESET}{color}{label}{RESET}");
        }
        // Unexpected non-"true" value on a flag key — fall through to
        // the raw renderer so we don't silently swallow the data.
    }
    // Value-bearing warning: a slow cable on a fast device is the app's
    // headline use case — same yellow as the cable.trust.* flags.
    if key == "cable.data_speed_limit" {
        return writeln!(
            w,
            "  {DIM}• {RESET}{YELLOW}Cable limits data to {value}{RESET}"
        );
    }
    writeln!(w, "  {DIM}• {RESET}{}: {value}", property_label(key))
}

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
        // Render the typed top-level fields first — link speed, version,
        // VID:PID — then the looser properties bag.
        if dev.link_speed_mbps > 0 {
            let label = usbeehive::usb::link_speed_tier(dev.link_speed_mbps).label();
            writeln!(w, "  {DIM}• {RESET}{label}")?;
        }
        if !dev.usb_version.is_empty() {
            writeln!(w, "  {DIM}• {RESET}USB {}", dev.usb_version)?;
        }
        if dev.vendor_id != 0 || dev.product_id != 0 {
            writeln!(
                w,
                "  {DIM}• {RESET}VID:PID {:04x}:{:04x}",
                dev.vendor_id, dev.product_id
            )?;
        }
        if !dev.primary_driver.is_empty() {
            writeln!(w, "  {DIM}• {RESET}Driver: {}", dev.primary_driver)?;
        }
        write_power_lines(w, &dev.power)?;
        for (key, value) in &dev.properties {
            // `usb_max_power_ma` is shown in mA — the daemon emits the raw
            // descriptor value (bMaxPower), so don't multiply.
            write_property(w, key, value)?;
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
    let roots = tree_roots(usb);
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
    write_power_lines(w, &dev.power)?;
    for (key, value) in &dev.properties {
        write_property(w, key, value)?;
    }
    Ok(())
}

/// Render the inbound/outbound power bullets. PD wattages are negotiated
/// ceilings (UCSI reports the RDO operating point, never measured flow),
/// so every figure carries "up to"; when the active contract allows more
/// than the sink requests, the contract is shown alongside so a low
/// number reads as sink policy rather than a bad cable.
fn write_power_lines<W: Write>(w: &mut W, power: &PowerSummary) -> io::Result<()> {
    if power.power_in_mw > 0 {
        let w_in = power.power_in_mw / 1000;
        let contract = power.contract_mw / 1000;
        if contract > w_in {
            writeln!(
                w,
                "  {DIM}• {RESET}Charging in: up to {w_in}W ({contract}W contract)"
            )?;
        } else {
            writeln!(w, "  {DIM}• {RESET}Charging in: up to {w_in}W")?;
        }
    }
    if power.power_out_mw > 0 {
        let w_out = power.power_out_mw / 1000;
        writeln!(w, "  {DIM}• {RESET}Sourcing out: up to {w_out}W")?;
    }
    Ok(())
}

fn print_root<W: Write>(w: &mut W, dev: &UsbDevice) -> io::Result<()> {
    let color = speed_color(dev.link_speed_tier());
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
    let color = speed_color(dev.link_speed_tier());
    let style = if dev.is_hub { ITALIC } else { "" };
    let name = dev.display_name();

    let mut hint = String::new();
    if !dev.is_hub && name == parent.display_name() {
        let cls = first_class_name(dev);
        if !cls.is_empty() {
            hint = format!("  {DIM}({cls}){RESET}");
        }
    }

    writeln!(w, "{prefix}{color}{connector}{style}{name}{RESET}{hint}")?;

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

fn speed_color(tier: LinkSpeed) -> &'static str {
    // Low Speed and Unknown both bucket to gray. Some Low-Speed devices
    // report `speed = "1.5"` which fails int parse and falls into Unknown.
    match tier {
        LinkSpeed::Usb4 => BRIGHT_MAGENTA,
        LinkSpeed::SuperPlus20 => MAGENTA,
        LinkSpeed::SuperPlus => BRIGHT_BLUE,
        LinkSpeed::Super => CYAN,
        LinkSpeed::High => GREEN,
        LinkSpeed::Full => YELLOW,
        LinkSpeed::Low | LinkSpeed::Unknown => BRIGHT_BLACK,
    }
}

fn root_label(d: &UsbDevice) -> &'static str {
    match d.link_speed_tier() {
        LinkSpeed::Usb4 => "USB4 root",
        LinkSpeed::SuperPlus20 => "USB 3.2 root",
        LinkSpeed::SuperPlus => "USB 3.1 root",
        LinkSpeed::Super => "USB 3.0 root",
        LinkSpeed::High => "USB 2.0 root",
        LinkSpeed::Full => "USB 1.1 root",
        LinkSpeed::Low | LinkSpeed::Unknown => "USB root",
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
    obj.insert(
        "deviceClass".into(),
        Value::String(format!("{:?}", dev.device_class)),
    );
    obj.insert(
        "deviceSubclass".into(),
        Value::String(dev.device_subclass.clone()),
    );
    obj.insert("status".into(), Value::String(format!("{:?}", dev.status)));
    obj.insert("headline".into(), Value::String(dev.headline.clone()));
    obj.insert("subtitle".into(), Value::String(dev.subtitle.clone()));
    obj.insert("icon".into(), Value::String(dev.icon.clone()));
    obj.insert("vendor".into(), Value::String(dev.vendor.clone()));
    obj.insert("product".into(), Value::String(dev.product.clone()));
    obj.insert("vendorId".into(), Value::String(hex_vidpid(dev.vendor_id)));
    obj.insert(
        "productId".into(),
        Value::String(hex_vidpid(dev.product_id)),
    );
    obj.insert(
        "primaryDriver".into(),
        Value::String(dev.primary_driver.clone()),
    );
    obj.insert(
        "linkSpeedMbps".into(),
        Value::Number(dev.link_speed_mbps.into()),
    );
    obj.insert("usbVersion".into(), Value::String(dev.usb_version.clone()));
    obj.insert(
        "power".into(),
        json!({
            "powerInMW": dev.power.power_in_mw,
            "powerOutMW": dev.power.power_out_mw,
            "contractMW": dev.power.contract_mw,
            "powerRole": format!("{:?}", dev.power.power_role),
        }),
    );
    // Properties: array of [key, value] tuples — mirrors the D-Bus a(ss)
    // wire shape so jq users can use the same key vocabulary.
    obj.insert(
        "properties".into(),
        Value::Array(
            dev.properties
                .iter()
                .map(|(k, v)| {
                    Value::Array(vec![Value::String(k.clone()), Value::String(v.clone())])
                })
                .collect(),
        ),
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
    use usbeehive::summary::Status;
    use usbeehive::usb::UsbDevice;

    #[test]
    fn empty_text_says_no_devices() {
        let mut buf = Vec::new();
        print_text_iter(&mut buf, &[], false).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("No USB devices found."));
    }

    fn render_property_string(key: &str, value: &str) -> String {
        let mut buf = Vec::new();
        write_property(&mut buf, key, value).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn flag_properties_render_label_only_when_true() {
        // `transport.*` and `cable.trust.*` are boolean flags — the
        // renderer should drop the trailing ": true".
        let out = render_property_string("transport.usb4", "true");
        assert!(out.contains("USB4 link"), "{out}");
        assert!(!out.contains("true"), "{out}");

        let out = render_property_string("transport.dp_altmode", "true");
        assert!(out.contains("DisplayPort altmode"), "{out}");

        let out = render_property_string("cable.trust.zero_vid", "true");
        assert!(out.contains("zero VID"), "{out}");
        // Trust flags carry the YELLOW warning colour code.
        assert!(out.contains(YELLOW), "{out}");

        // The no-e-marker hint is informational — label only, no yellow.
        let out = render_property_string("cable.no_emarker", "true");
        assert!(out.contains("No cable e-marker visible"), "{out}");
        assert!(!out.contains(YELLOW), "{out}");
    }

    #[test]
    fn value_bearing_properties_keep_colon_format() {
        let out = render_property_string("charger_max", "100W");
        assert!(out.contains("Charger max: 100W"), "{out}");

        let out = render_property_string("cable_speed", "USB 3.2 Gen 2");
        assert!(out.contains("Cable speed: USB 3.2 Gen 2"), "{out}");
    }

    #[test]
    fn data_speed_limit_renders_yellow_sentence() {
        // Value-bearing warning — prose form, yellow like cable.trust.*.
        let out = render_property_string("cable.data_speed_limit", "USB 2.0");
        assert!(out.contains("Cable limits data to USB 2.0"), "{out}");
        assert!(out.contains(YELLOW), "{out}");
    }

    #[test]
    fn unknown_property_falls_through_to_raw_key() {
        let out = render_property_string("future.unknown.key", "42");
        assert!(out.contains("future.unknown.key: 42"), "{out}");
    }

    #[test]
    fn flag_property_with_non_true_value_falls_through() {
        // Defensive: if the daemon ever emits a non-"true" value on a flag
        // key, we should print it raw rather than silently dropping data.
        let out = render_property_string("transport.usb4", "partial");
        assert!(out.contains("transport.usb4: partial"), "{out}");
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
        let port = usbeehive::typec::TypeCPort {
            port_number: 0,
            data_role: "host [device]".into(),
            ..Default::default()
        };
        let s = DeviceSummary::from_typec_port(&port, None, None, None);
        assert_eq!(s.status, Status::Empty);
        let v = device_json(&s, false);
        assert_eq!(v["category"], "typec");
        assert_eq!(v["typec"]["dataRole"], "device");
    }

    #[test]
    fn speed_color_buckets() {
        assert_eq!(speed_color(LinkSpeed::Unknown), BRIGHT_BLACK);
        assert_eq!(speed_color(LinkSpeed::Low), BRIGHT_BLACK);
        assert_eq!(speed_color(LinkSpeed::Full), YELLOW);
        assert_eq!(speed_color(LinkSpeed::High), GREEN);
        assert_eq!(speed_color(LinkSpeed::Super), CYAN);
        assert_eq!(speed_color(LinkSpeed::SuperPlus), BRIGHT_BLUE);
        assert_eq!(speed_color(LinkSpeed::SuperPlus20), MAGENTA);
        assert_eq!(speed_color(LinkSpeed::Usb4), BRIGHT_MAGENTA);
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
    fn print_tree_legend_appears_when_devices_exist() {
        use usbeehive::DeviceManager;
        use usbeehive::Sysfs;
        let mgr = DeviceManager::with_sysfs(Sysfs::with_root("/no/such/usbeehive/path"));
        let mut buf = Vec::new();
        print_tree(&mut buf, &mgr).unwrap();
        let out = String::from_utf8_lossy(&buf);
        assert!(out.contains("No USB devices found."));
    }

    #[test]
    fn text_output_renders_charger_profiles() {
        use usbeehive::power::{PdoType, PowerDataObject, PowerDeliveryPort};
        use usbeehive::typec::{TypeCPartner, TypeCPort};
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
        let s = DeviceSummary::from_typec_port(&port, Some(pd), None, None);
        let mut buf = Vec::new();
        print_text_iter(&mut buf, &[s], false).unwrap();
        let out = String::from_utf8_lossy(&buf);
        assert!(out.contains("Charger profiles:"));
        assert!(out.contains("20.0V @ 5.00A"));
        assert!(out.contains("◀ active"));
    }
}
