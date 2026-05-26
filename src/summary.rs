//! Plain-data summary of a USB device or Type-C port.
//!
//! [`DeviceSummary`] is the structured façade most consumers reach for. It
//! carries a coarse classification (`category`, `device_class`,
//! `device_subclass`, `status`), the universal identity fields
//! (`vendor`, `product`, `vendor_id`, `product_id`, `primary_driver`), the
//! per-port/per-device transport stats (`link_speed_mbps`, `usb_version`,
//! `power`, `charging_diag`), and a `properties` list of `(machine_key,
//! value)` pairs for everything that doesn't fit a typed field.
//!
//! Owner of English display prose is the CLI text renderer
//! (`src/output.rs`) — `DeviceSummary` carries machine keys only. UI
//! clients translate keys to localized strings.
//!
//! Enums are extensible: adding a new `DeviceClass` / `Status` /
//! `PowerRole` variant is a non-breaking change. Consumers MUST treat
//! any unrecognized variant as `Unknown` / fall back to category-based
//! behavior.

use serde::Serialize;

use crate::cable::CableInfo;
use crate::diagnostic::ChargingDiagnostic;
use crate::pd::{decode_id_header, product_type_label};
use crate::power::PowerDeliveryPort;
use crate::typec::{TypeCPort, TypeCPowerSupply};
use crate::usb::{UsbDevice, UsbInterface};
use crate::usbclass;
use crate::vendor;

/// High-level grouping a [`DeviceSummary`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    /// Plain USB peripheral.
    UsbDevice,
    /// USB Type-C port (with or without a partner attached).
    TypeCPort,
    /// USB hub (`device_class == 0x09`).
    Hub,
}

/// Connection / power flow status surfaced to the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Status {
    /// Type-C port with nothing attached.
    Empty,
    /// Attached (no active PD contract, or USB device).
    Connected,
    /// Attached and we are sinking PD power from this port.
    Charging,
    /// Attached and we are sourcing PD power through this port to a
    /// downstream device (e.g. host charging a phone).
    Sourcing,
}

/// Coarse device classification for UI badging / sorting / search.
///
/// Adding a new variant is non-breaking. UI consumers must treat any
/// unrecognized value as [`Self::Unknown`]. When [`DeviceSummary::category`]
/// is [`Category::TypeCPort`], `device_class` is always [`Self::Unknown`]
/// — Type-C ports don't get a device class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeviceClass {
    /// USB-IF HID keyboard.
    Keyboard,
    /// USB-IF HID mouse.
    Mouse,
    /// HID device with `tablet` / `digitizer` / `pen` in the product name.
    InputTablet,
    /// HID device with `gamepad` / `joystick` / `controller` in the product name.
    Gamepad,
    /// USB-IF Mass Storage (0x08).
    Storage,
    /// USB-IF Video class with display-like product naming. Reserved —
    /// day-one fidelity is low; unmatched devices fall to [`Self::Unknown`].
    Display,
    /// USB-IF Audio (0x01).
    Audio,
    /// USB-IF Video (0x0E), camera variant.
    Camera,
    /// USB-IF Video (0x0E) with product-string match for capture cards.
    VideoCapture,
    /// USB-IF Printer (0x07).
    Printer,
    /// iPhones (Apple VID + product) and Android handsets — detected via
    /// product-string `android`, ADB/PTP+vendor-class interface signatures,
    /// or a phone-VID allowlist paired with PTP / MTP-shaped functions.
    Phone,
    /// USB-IF Hub (0x09).
    Hub,
    /// CDC Ethernet (0x02/0x06) or vendor-specific NIC driver.
    NetworkWired,
    /// USB-IF Wireless (0xE0) excluding Bluetooth subclass.
    NetworkWireless,
    /// FIDO / U2F / smartcard token (Yubico, Nitrokey, Titan, Solo, …).
    SecurityKey,
    /// USB-IF Smart Card / CCID (0x0B).
    SmartcardReader,
    /// USB-IF Wireless (0xE0) Radio Frequency subclass 0x01 / protocol 0x01.
    Bluetooth,
    /// CDC ACM (0x02/0x02) or vendor-specific USB-UART
    /// (FTDI, CP210x, CH340/CH341, PL2303, TI, MOS).
    Serial,
    /// Fallthrough — classification could not narrow the device.
    Unknown,
}

/// PD power-role for a Type-C port. Reflects the **current contract
/// direction** when one is active; otherwise reflects capability.
///
/// - `Sink` / `Source` — active contract in that direction
/// - `DualRole` — port is dual-role-capable but no active contract
/// - `Unknown` — port has no PD info (e.g. a plain USB-A device)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PowerRole {
    /// We are sourcing power out through this port.
    Source,
    /// We are sinking power in through this port (i.e. being charged).
    Sink,
    /// Port is dual-role-capable, no active contract.
    DualRole,
    /// No PD information.
    Unknown,
}

/// Per-entry power flow summary.
///
/// Invariant: `power_in_mw > 0` ⟺ "this port is actively sinking PD
/// power right now". `power_out_mw > 0` ⟺ "this port is actively
/// sourcing PD power right now". Both zero for non-PD-capable entries
/// (plain USB devices, hubs). For plain USB device descriptor draw see
/// the `usb_power_ma` entry in [`DeviceSummary::properties`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PowerSummary {
    /// Power flowing into the host from this port, in milliwatts. Zero
    /// when not sinking, or for non-PD entries.
    pub power_in_mw: u32,
    /// Power flowing out of the host through this port, in milliwatts.
    /// Zero when not sourcing.
    pub power_out_mw: u32,
    /// Current contract direction (or capability when no contract).
    pub power_role: PowerRole,
}

impl Default for PowerSummary {
    fn default() -> Self {
        PowerSummary {
            power_in_mw: 0,
            power_out_mw: 0,
            power_role: PowerRole::Unknown,
        }
    }
}

/// One renderable summary entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceSummary {
    /// High-level grouping for color/iconography.
    pub category: Category,
    /// Coarse device classification. [`DeviceClass::Unknown`] when this
    /// is a [`Category::TypeCPort`] or classification couldn't narrow.
    pub device_class: DeviceClass,
    /// Optional fine-grain class hint (`"webcam"` / `"capture"` /
    /// `"sd_reader"` / …). Advisory only; UIs that don't care ignore it.
    /// Empty string by default. Never carries security-relevant info.
    pub device_subclass: String,
    /// Connection / power flow state.
    pub status: Status,
    /// Single-line title (e.g. `"USB-C Port 0"` or the product string).
    pub headline: String,
    /// Single-line subtitle (vendor + class).
    pub subtitle: String,
    /// Suggested freedesktop icon name.
    pub icon: String,
    /// USB descriptor `iManufacturer` (or vendor-DB fallback). Empty
    /// when both fail.
    pub vendor: String,
    /// USB descriptor `iProduct`. Empty when unset.
    pub product: String,
    /// `idVendor`. Zero for non-USB entries.
    pub vendor_id: u16,
    /// `idProduct`. Zero for non-USB entries.
    pub product_id: u16,
    /// Kernel driver bound to the device's first interface. Empty when
    /// no driver is bound — a meaningful UI signal.
    pub primary_driver: String,
    /// `(machine_key, value)` pairs for attributes that don't fit a typed
    /// field. Adding new keys is non-breaking; renaming or removing is
    /// breaking. Key vocabulary is daemon-owned and documented in
    /// CHANGELOG.
    pub properties: Vec<(String, String)>,
    /// Negotiated link speed in **Mbps**. Zero when unknown.
    pub link_speed_mbps: u32,
    /// Canonical USB version short form (`"2.0"`, `"3.2"`, `"4.0"`).
    /// Empty when unknown.
    pub usb_version: String,
    /// Power flow summary.
    pub power: PowerSummary,
    /// Computed charging diagnostic. Present only when the port is
    /// actively sinking PD power.
    pub charging_diag: Option<ChargingDiagnostic>,

    /// Original USB device, if this summary describes one.
    pub usb_device: Option<UsbDevice>,
    /// Original Type-C port, if this summary describes one.
    pub typec_port: Option<TypeCPort>,
    /// Companion PD port for `typec_port`.
    pub power_delivery: Option<PowerDeliveryPort>,
    /// Decoded cable info attached to `typec_port`.
    pub cable: Option<CableInfo>,
}

fn canonical_usb_version(bcd: &str) -> String {
    // bcdUSB sysfs strings are decimal-formatted like "2.10", "3.20", "4.00".
    // The `.NN` part is *decimal subversion* in USB-IF convention, not a
    // fraction — "2.10" means USB 2.1, "3.20" means USB 3.2. Round-trip
    // through f32 to drop the trailing zero while preserving "2.0" / "4.0".
    if bcd.is_empty() {
        return String::new();
    }
    bcd.parse::<f32>()
        .map(|v| format!("{v:.1}"))
        .unwrap_or_default()
}

fn power_contract_string(psy: &TypeCPowerSupply) -> Option<String> {
    let _mw = psy.negotiated_power_mw()?;
    let v_uv = psy.voltage_now_uv?;
    let i_ua = psy.current_now_ua?;
    let volts = v_uv as f64 / 1_000_000.0;
    let amps = i_ua as f64 / 1_000_000.0;
    Some(format!("{volts:.1}V @ {amps:.2}A"))
}

fn is_security_key(vendor_id: u16, product_lower: &str) -> bool {
    // Yubico, Nitrokey by VID; everyone else by product-string heuristic.
    if vendor_id == 0x1050 || vendor_id == 0x20A0 {
        return true;
    }
    [
        "yubikey",
        "security key",
        "nitrokey",
        "solokey",
        "onlykey",
        "titan security",
        "fido",
    ]
    .iter()
    .any(|needle| product_lower.contains(needle))
}

/// Known phone-maker USB vendor IDs. Used as a fallback signal when a
/// composite advertises an MTP/PTP function but no ADB function — most real
/// phones do, modulo developer-mode off.
const PHONE_VIDS: &[u16] = &[
    0x18D1, // Google
    0x04E8, // Samsung
    0x22B8, // Motorola
    0x0FCE, // Sony Mobile
    0x1BBB, // Bullitt / Cat
    0x2A70, // OnePlus
    0x2717, // Xiaomi
    0x12D1, // Huawei
    0x19D2, // ZTE
    0x0BB4, // HTC
    0x1004, // LG Electronics
    0x0E8D, // MediaTek (used by many phone OEMs)
    0x2A45, // Meizu
    0x2916, // Oppo
    0x05C6, // Qualcomm (used by many Android composites)
];

fn iface_matches(ifaces: &[UsbInterface], class: u8, sub: u8, proto: u8) -> bool {
    ifaces
        .iter()
        .any(|i| i.class_code == class && i.sub_class == sub && i.protocol == proto)
}

fn is_phone(vendor_id: u16, product_lower: &str, ifaces: &[UsbInterface]) -> bool {
    if vendor_id == 0x05AC && product_lower.contains("iphone") {
        return true;
    }
    if product_lower.contains("android") {
        return true;
    }
    // ADB function signature. No camera, printer, or UART bridge uses this
    // triple — it's a strong positive indicator of an Android composite.
    if iface_matches(ifaces, 0xFF, 0x42, 0x01) {
        return true;
    }
    let has_ptp = iface_matches(ifaces, 0x06, 0x01, 0x01);
    // MTP isn't a USB-IF class — it's typically advertised as vendor-specific
    // (0xFF). Phones that expose MTP without ADB show a 0xFF interface
    // alongside the PTP one.
    let has_vendor_specific = ifaces.iter().any(|i| i.class_code == 0xFF);
    // PTP-only is a DSLR (Canon, Nikon). PTP paired with vendor-specific is
    // the Android composite shape.
    if has_ptp && has_vendor_specific {
        return true;
    }
    // Phone-VID fallback: catches Android phones that don't expose ADB and
    // are PTP-only (e.g. Cat S61 in MTP/Photo Transfer modes).
    if PHONE_VIDS.contains(&vendor_id) && (has_ptp || has_vendor_specific) {
        return true;
    }
    false
}

fn is_capture_card(product_lower: &str) -> bool {
    [
        "capture", "hdmi", "elgato", "cam link", "magewell", "av.io", "epiphan",
    ]
    .iter()
    .any(|needle| product_lower.contains(needle))
}

/// Drivers commonly bound to USB-to-UART vendor-specific bridges.
/// Matched against `UsbDevice.primary_driver()` to classify [`DeviceClass::Serial`].
const SERIAL_DRIVERS: &[&str] = &[
    "ftdi_sio",
    "cp210x",
    "ch341",
    "ch340",
    "pl2303",
    "ti_usb_3410_5052",
    "mos7720",
    "mos7840",
    "ark3116",
    "io_ti",
];

/// Drivers commonly bound to USB-to-Ethernet vendor-specific NICs.
const ETHERNET_DRIVERS: &[&str] = &[
    "cdc_ether",
    "r8152",
    "asix",
    "ax88179_178a",
    "ax88172a",
    "rndis_host",
    "smsc75xx",
    "smsc95xx",
    "lan78xx",
];

fn primary_driver(dev: &UsbDevice) -> String {
    dev.interfaces
        .iter()
        .find(|i| !i.driver.is_empty())
        .map(|i| i.driver.clone())
        .unwrap_or_default()
}

fn unique_drivers(dev: &UsbDevice) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for iface in &dev.interfaces {
        if !iface.driver.is_empty() && !out.contains(&iface.driver) {
            out.push(iface.driver.clone());
        }
    }
    out
}

/// Classify a [`UsbDevice`] into a coarse [`DeviceClass`] plus optional
/// subclass hint. Reads interface class/subclass/protocol bytes plus
/// vendor + product string heuristics. See CONTEXT.md for the day-one
/// fidelity matrix.
pub fn classify_usb(dev: &UsbDevice) -> (DeviceClass, String) {
    if dev.is_hub {
        return (DeviceClass::Hub, String::new());
    }

    let product_lower = dev.product.to_ascii_lowercase();

    // SecurityKey checked before HID — FIDO tokens advertise HID interfaces.
    if is_security_key(dev.vendor_id, &product_lower) {
        return (DeviceClass::SecurityKey, String::new());
    }
    if is_phone(dev.vendor_id, &product_lower, &dev.interfaces) {
        return (DeviceClass::Phone, String::new());
    }

    let driver = primary_driver(dev);
    let driver_l = driver.to_ascii_lowercase();

    // Interface-class scan. First match wins — order is important for
    // overlapping cases (e.g. CCID + HID is a smartcard, not a keyboard).
    let mut hid_keyboard = false;
    let mut hid_mouse = false;
    let mut hid_other = false;
    for iface in &dev.interfaces {
        match iface.class_code {
            0x0B => return (DeviceClass::SmartcardReader, String::new()),
            0x07 => return (DeviceClass::Printer, String::new()),
            0x08 => {
                let sub = match iface.sub_class {
                    0x06 => "scsi",
                    0x05 => "sff8070i",
                    0x04 => "ufi",
                    0x02 => "atapi",
                    _ => "",
                };
                return (DeviceClass::Storage, sub.into());
            }
            0x01 => {
                let sub = if product_lower.contains("microphone") || product_lower.contains("mic ")
                {
                    "microphone"
                } else if product_lower.contains("headset")
                    || product_lower.contains("headphone")
                    || product_lower.contains("earbud")
                    || product_lower.contains("airpod")
                {
                    "headset"
                } else if product_lower.contains("dac") || product_lower.contains("interface") {
                    "dac"
                } else {
                    ""
                };
                return (DeviceClass::Audio, sub.into());
            }
            0x0E => {
                if is_capture_card(&product_lower) {
                    return (DeviceClass::VideoCapture, "capture".into());
                }
                return (DeviceClass::Camera, "webcam".into());
            }
            0xE0 => {
                // Wireless. Subclass 0x01 = Radio Frequency. Protocol 0x01 = Bluetooth.
                if iface.sub_class == 0x01 && iface.protocol == 0x01 {
                    return (DeviceClass::Bluetooth, String::new());
                }
                return (DeviceClass::NetworkWireless, String::new());
            }
            0x02 => {
                // CDC. Subclass 0x06 = Ethernet, 0x02 = ACM (serial).
                match iface.sub_class {
                    0x06 => return (DeviceClass::NetworkWired, String::new()),
                    0x02 => return (DeviceClass::Serial, String::new()),
                    _ => {}
                }
            }
            0x03 => {
                // HID. Protocol 0x01 = keyboard, 0x02 = mouse. Tablets /
                // gamepads use product-string heuristics on the boot protocol 0.
                match iface.protocol {
                    0x01 => hid_keyboard = true,
                    0x02 => hid_mouse = true,
                    _ => hid_other = true,
                }
            }
            _ => {}
        }
    }

    // Vendor-specific driver allowlists for serial UARTs and NICs (FTDI,
    // CP210x, CH340, etc. don't advertise CDC).
    if SERIAL_DRIVERS.iter().any(|d| driver_l == *d) {
        return (DeviceClass::Serial, String::new());
    }
    if ETHERNET_DRIVERS.iter().any(|d| driver_l == *d) {
        return (DeviceClass::NetworkWired, String::new());
    }

    if hid_keyboard {
        return (DeviceClass::Keyboard, String::new());
    }
    if hid_mouse {
        return (DeviceClass::Mouse, String::new());
    }
    if hid_other {
        if ["tablet", "digitizer", "pen", "wacom"]
            .iter()
            .any(|n| product_lower.contains(n))
        {
            return (DeviceClass::InputTablet, String::new());
        }
        if ["gamepad", "joystick", "controller", "xbox", "dualshock"]
            .iter()
            .any(|n| product_lower.contains(n))
        {
            return (DeviceClass::Gamepad, String::new());
        }
    }

    (DeviceClass::Unknown, String::new())
}

fn icon_for_class(class: DeviceClass) -> &'static str {
    match class {
        DeviceClass::Keyboard => "input-keyboard",
        DeviceClass::Mouse => "input-mouse",
        DeviceClass::InputTablet => "input-tablet",
        DeviceClass::Gamepad => "input-gaming",
        DeviceClass::Storage => "drive-removable-media",
        DeviceClass::Display => "video-display",
        DeviceClass::Audio => "audio-card",
        DeviceClass::Camera => "camera-web",
        DeviceClass::VideoCapture => "camera-video",
        DeviceClass::Printer => "printer",
        DeviceClass::Phone => "phone",
        DeviceClass::Hub => "network-wired",
        DeviceClass::NetworkWired => "network-wired",
        DeviceClass::NetworkWireless => "network-wireless",
        DeviceClass::SecurityKey => "auth-smartcard",
        DeviceClass::SmartcardReader => "auth-smartcard",
        DeviceClass::Bluetooth => "bluetooth",
        DeviceClass::Serial => "utilities-terminal",
        DeviceClass::Unknown => "drive-removable-media-usb",
    }
}

fn refine_audio_icon(class: DeviceClass, subclass: &str) -> &'static str {
    if class == DeviceClass::Audio
        && (subclass == "headset" || subclass.contains("headphone") || subclass == "earbud")
    {
        "audio-headphones"
    } else {
        icon_for_class(class)
    }
}

impl DeviceSummary {
    /// Stable identifier suitable for diffing two snapshots.
    pub fn id(&self) -> String {
        if let Some(p) = &self.typec_port {
            format!("typec:{}", p.port_name)
        } else if let Some(d) = &self.usb_device {
            format!("usb:{}", d.bus_port)
        } else {
            String::new()
        }
    }

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

        let (device_class, device_subclass) = classify_usb(dev);
        let icon = refine_audio_icon(device_class, &device_subclass).to_string();

        let vendor_display = if !dev.manufacturer.is_empty() {
            dev.manufacturer.clone()
        } else if has_vendor {
            vendor_name.clone()
        } else {
            String::new()
        };

        let primary = primary_driver(dev);

        // Properties — machine keys per CONTEXT.md.
        let mut properties: Vec<(String, String)> = Vec::new();
        if !dev.serial.is_empty() {
            properties.push(("serial".into(), dev.serial.clone()));
        }
        match dev.removable.as_str() {
            "removable" | "fixed" => properties.push(("mount".into(), dev.removable.clone())),
            _ => {}
        }
        let drivers = unique_drivers(dev);
        if drivers.len() > 1 {
            properties.push(("drivers".into(), drivers.join(", ")));
        }
        if dev.max_power_ma > 0 {
            properties.push(("usb_power_ma".into(), dev.max_power_ma.to_string()));
        }

        DeviceSummary {
            category: if dev.is_hub {
                Category::Hub
            } else {
                Category::UsbDevice
            },
            device_class,
            device_subclass,
            status: Status::Connected,
            headline: dev.display_name(),
            subtitle,
            icon,
            vendor: vendor_display,
            product: dev.product.clone(),
            vendor_id: dev.vendor_id,
            product_id: dev.product_id,
            primary_driver: primary,
            properties,
            link_speed_mbps: dev.speed,
            usb_version: canonical_usb_version(&dev.version),
            power: PowerSummary::default(),
            charging_diag: None,
            usb_device: Some(dev.clone()),
            typec_port: None,
            power_delivery: None,
            cable: None,
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
            device_class: DeviceClass::Unknown,
            device_subclass: String::new(),
            status: Status::Empty,
            headline: format!("USB-C Port {}", port.port_number),
            subtitle: String::new(),
            icon: "plug".into(),
            vendor: String::new(),
            product: String::new(),
            vendor_id: 0,
            product_id: 0,
            primary_driver: String::new(),
            properties: Vec::new(),
            link_speed_mbps: 0,
            usb_version: String::new(),
            power: PowerSummary::default(),
            charging_diag: None,
            usb_device: None,
            typec_port: Some(port.clone()),
            power_delivery: pd,
            cable: cable_info,
        };

        if !port.is_connected() {
            s.subtitle = "Nothing connected".into();
            return s;
        }
        s.status = Status::Connected;

        if let Some(partner) = &port.partner {
            if let Some(&vdo) = partner.identity.as_ref().and_then(|id| id.vdos.first()) {
                let hdr = decode_id_header(vdo);
                let product_label = product_type_label(
                    hdr.ufp_product_type
                        .unwrap_or(crate::pd::ProductType::Undefined),
                );
                let vendor_label = vendor::lookup(hdr.vendor_id);
                s.vendor_id = hdr.vendor_id;
                if vendor::is_hex_fallback(&vendor_label) {
                    s.subtitle = product_label.to_string();
                } else {
                    s.vendor = vendor_label.clone();
                    s.subtitle = format!("{vendor_label} — {product_label}");
                }
            } else {
                s.subtitle = "Device connected".into();
            }
        }

        let data = port.current_data_role();
        let power_role_str = port.current_power_role();
        if !data.is_empty() {
            s.properties.push(("data_role".into(), data));
        }
        if !port.power_op_mode.is_empty() {
            s.properties
                .push(("power_mode".into(), port.power_op_mode.clone()));
        }

        // Live PD contract — only when actually online.
        if let Some(psy) = &s.typec_port.as_ref().unwrap().power_supply {
            if psy.online {
                if let Some(c) = power_contract_string(psy) {
                    s.properties.push(("pd_contract".into(), c));
                }
            }
        }

        if !port.pd_revision.is_empty() {
            s.properties
                .push(("pd_revision".into(), port.pd_revision.clone()));
        }
        if !port.orientation.is_empty() && port.orientation != "unknown" {
            s.properties
                .push(("plug_orientation".into(), port.orientation.clone()));
        }

        if let Some(c) = &s.cable {
            if let Some(speed) = c.speed {
                s.properties.push((
                    "cable_speed".into(),
                    crate::pd::cable_speed_label(speed).into(),
                ));
            }
            if let Some(curr) = c.current_rating {
                s.properties.push((
                    "cable_current".into(),
                    crate::pd::cable_current_label(curr).into(),
                ));
            }
            if c.max_watts > 0 {
                s.properties
                    .push(("cable_max_power".into(), format!("{}W", c.max_watts)));
            }
            if c.is_active {
                s.properties.push(("cable_type".into(), "active".into()));
            } else if c.is_passive {
                s.properties.push(("cable_type".into(), "passive".into()));
            }
            if !c.vendor_name.is_empty() && !vendor::is_hex_fallback(&c.vendor_name) {
                s.properties
                    .push(("cable_vendor".into(), c.vendor_name.clone()));
            }
            // Trust signals: push only the flags that fire — UI consumers
            // render badges per key, clean cables stay quiet.
            if c.trust.zero_vid {
                s.properties
                    .push(("cable.trust.zero_vid".into(), "true".into()));
            }
            if c.trust.vid_unknown {
                s.properties
                    .push(("cable.trust.vid_unknown".into(), "true".into()));
            }
            if c.trust.reserved_bits_set {
                s.properties
                    .push(("cable.trust.reserved_bits".into(), "true".into()));
            }
        }

        // PD source advertisement → sinking power (we're being charged).
        let mut sink_power_mw: u32 = 0;
        let mut source_power_mw: u32 = 0;
        if let Some(pd_port) = &s.power_delivery {
            if !pd_port.source_capabilities.is_empty() {
                let max_w = pd_port.max_source_power_mw / 1000;
                s.properties
                    .push(("charger_max".into(), format!("{max_w}W")));
                s.status = Status::Charging;
                // Active PDO ⇒ contracted power. Fall back to advertised max.
                sink_power_mw = pd_port
                    .source_capabilities
                    .iter()
                    .find(|p| p.is_active)
                    .map(|p| p.power_mw)
                    .unwrap_or(pd_port.max_source_power_mw);
            }
            // Live UCSI overrides if available.
            if let Some(psy) = &port.power_supply {
                if let Some(live_mw) = psy.negotiated_power_mw() {
                    if live_mw > 0 {
                        sink_power_mw = live_mw as u32;
                    }
                }
            }
            s.charging_diag = ChargingDiagnostic::evaluate(pd_port, s.cable.as_ref());
        }

        // Sourcing detection — power role is "source" with no source-caps
        // (we're not the charger, we're the source).
        if power_role_str.eq_ignore_ascii_case("source") && sink_power_mw == 0 {
            // Pull a rough estimate of out-bound power from the UCSI live
            // readout if available; else leave as zero and let the role flag carry.
            if let Some(psy) = &port.power_supply {
                if let Some(live_mw) = psy.negotiated_power_mw() {
                    if live_mw > 0 {
                        source_power_mw = live_mw as u32;
                    }
                }
            }
            s.status = Status::Sourcing;
        }

        let role = if sink_power_mw > 0 {
            PowerRole::Sink
        } else if source_power_mw > 0 {
            PowerRole::Source
        } else {
            match port.port_type.as_str() {
                "dual" | "drp" => PowerRole::DualRole,
                "source" => PowerRole::Source,
                "sink" => PowerRole::Sink,
                _ => match power_role_str.to_ascii_lowercase().as_str() {
                    "source" => PowerRole::Source,
                    "sink" => PowerRole::Sink,
                    _ => PowerRole::Unknown,
                },
            }
        };

        s.power = PowerSummary {
            power_in_mw: sink_power_mw,
            power_out_mw: source_power_mw,
            power_role: role,
        };

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
            manufacturer: "Apple".into(),
            version: "2.10".into(),
            speed: 480,
            max_power_ma: 500,
            device_class: 0,
            interfaces: vec![UsbInterface {
                class_code: 0x03,
                protocol: 0x01,
                driver: "usbhid".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn usb_summary_extracts_structured_fields() {
        let s = DeviceSummary::from_usb_device(&dev());
        assert_eq!(s.headline, "iPhone");
        assert_eq!(s.vendor, "Apple");
        assert_eq!(s.product, "iPhone");
        assert_eq!(s.vendor_id, 0x05AC);
        assert_eq!(s.product_id, 0x12A8);
        assert_eq!(s.primary_driver, "usbhid");
        assert_eq!(s.link_speed_mbps, 480);
        assert_eq!(s.usb_version, "2.1");
        assert_eq!(s.device_class, DeviceClass::Phone);
        assert_eq!(s.category, Category::UsbDevice);
        // usb_power_ma surfaces in properties, not in `power`.
        assert_eq!(s.power.power_in_mw, 0);
        assert!(s
            .properties
            .iter()
            .any(|(k, v)| k == "usb_power_ma" && v == "500"));
    }

    #[test]
    fn hub_class_marks_hub() {
        let mut d = dev();
        d.device_class = 0x09;
        d.is_hub = true;
        let s = DeviceSummary::from_usb_device(&d);
        assert_eq!(s.category, Category::Hub);
        assert_eq!(s.device_class, DeviceClass::Hub);
        assert_eq!(s.icon, "network-wired");
    }

    #[test]
    fn classify_keyboard_via_hid_protocol() {
        let d = UsbDevice {
            product: "Generic Keyboard".into(),
            interfaces: vec![UsbInterface {
                class_code: 0x03,
                protocol: 0x01,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Keyboard);
    }

    #[test]
    fn classify_mouse_via_hid_protocol() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0x03,
                protocol: 0x02,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Mouse);
    }

    #[test]
    fn classify_storage_via_class_code() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0x08,
                sub_class: 0x06,
                ..Default::default()
            }],
            ..Default::default()
        };
        let (class, sub) = classify_usb(&d);
        assert_eq!(class, DeviceClass::Storage);
        assert_eq!(sub, "scsi");
    }

    #[test]
    fn classify_smartcard_via_ccid_class() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0x0B,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::SmartcardReader);
    }

    #[test]
    fn classify_serial_via_cdc_acm() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0x02,
                sub_class: 0x02,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Serial);
    }

    #[test]
    fn classify_serial_via_ftdi_driver() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0xFF,
                driver: "ftdi_sio".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Serial);
    }

    #[test]
    fn classify_bluetooth_via_wireless_subclass() {
        let d = UsbDevice {
            interfaces: vec![UsbInterface {
                class_code: 0xE0,
                sub_class: 0x01,
                protocol: 0x01,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Bluetooth);
    }

    #[test]
    fn classify_security_key_by_vid() {
        // Security keys advertise HID — must take precedence over keyboard.
        let d = UsbDevice {
            vendor_id: 0x1050,
            interfaces: vec![UsbInterface {
                class_code: 0x03,
                protocol: 0x01,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::SecurityKey);
    }

    #[test]
    fn classify_phone_iphone_by_vid_and_product() {
        let d = UsbDevice {
            vendor_id: 0x05AC,
            product: "iPhone".into(),
            interfaces: vec![UsbInterface {
                class_code: 0xFF,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Phone);
    }

    #[test]
    fn classify_phone_android_via_adb_signature() {
        // Pixel-like composite with USB debugging enabled — ADB function
        // present alongside MTP (vendor-class) and an ACM serial diag
        // interface. The serial interface would otherwise win the inner
        // CDC-ACM branch and misclassify the device.
        let d = UsbDevice {
            vendor_id: 0x18D1,
            product: "Pixel 7".into(),
            interfaces: vec![
                UsbInterface {
                    class_code: 0xFF,
                    sub_class: 0x42,
                    protocol: 0x01,
                    ..Default::default()
                },
                UsbInterface {
                    class_code: 0x02,
                    sub_class: 0x02,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Phone);
    }

    #[test]
    fn classify_phone_cat_s61_via_vid_and_ptp() {
        // Cat S61 (Bullitt 0x1BBB). iProduct is the model name — no `android`
        // substring — and the device exposes PTP + a CDC-ACM diag function.
        // Without the VID+PTP fallback this gets classified as Serial.
        let d = UsbDevice {
            vendor_id: 0x1BBB,
            product: "Cat S61".into(),
            interfaces: vec![
                UsbInterface {
                    class_code: 0x06,
                    sub_class: 0x01,
                    protocol: 0x01,
                    ..Default::default()
                },
                UsbInterface {
                    class_code: 0x02,
                    sub_class: 0x02,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::Phone);
    }

    #[test]
    fn classify_camera_not_misidentified_as_phone() {
        // DSLR exposing pure PTP (no vendor-class interface, unknown VID).
        // Must classify as Camera, not Phone — the heuristic threshold is
        // PTP + vendor-class, OR phone-VID + PTP.
        let d = UsbDevice {
            vendor_id: 0x04A9, // Canon
            product: "EOS R5".into(),
            interfaces: vec![UsbInterface {
                class_code: 0x06,
                sub_class: 0x01,
                protocol: 0x01,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_ne!(classify_usb(&d).0, DeviceClass::Phone);
    }

    #[test]
    fn classify_capture_card_product_heuristic() {
        let d = UsbDevice {
            product: "Elgato Cam Link 4K".into(),
            interfaces: vec![UsbInterface {
                class_code: 0x0E,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(classify_usb(&d).0, DeviceClass::VideoCapture);
    }

    #[test]
    fn canonical_usb_version_normalizes_bcd() {
        assert_eq!(canonical_usb_version(""), "");
        assert_eq!(canonical_usb_version("2.10"), "2.1");
        assert_eq!(canonical_usb_version("3.20"), "3.2");
        assert_eq!(canonical_usb_version("4.00"), "4.0");
        assert_eq!(canonical_usb_version("2.00"), "2.0");
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
        assert_eq!(s.power.power_role, PowerRole::Unknown);
    }

    #[test]
    fn charging_status_when_pd_source_present() {
        use crate::power::{PdoType, PowerDataObject};
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
        assert!(s
            .properties
            .iter()
            .any(|(k, v)| k == "charger_max" && v == "100W"));
        assert!(s.charging_diag.is_some());
        assert_eq!(s.power.power_role, PowerRole::Sink);
        assert_eq!(s.power.power_in_mw, 100_000);
        assert_eq!(s.power.power_out_mw, 0);
    }

    #[test]
    fn power_contract_string_renders_volts_amps() {
        let psy = TypeCPowerSupply {
            online: true,
            voltage_now_uv: Some(9_000_000),
            current_now_ua: Some(2_000_000),
            ..Default::default()
        };
        let label = power_contract_string(&psy).unwrap();
        assert!(label.contains("9.0V"));
        assert!(label.contains("2.00A"));
    }

    #[test]
    fn cable_properties_appear() {
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
        let p: std::collections::HashMap<_, _> = s
            .properties
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert!(p
            .get("cable_speed")
            .is_some_and(|v| v.contains("3.2 Gen 2")));
        assert_eq!(p.get("cable_current").copied(), Some("5A"));
        assert_eq!(p.get("cable_max_power").copied(), Some("100W"));
        assert_eq!(p.get("cable_type").copied(), Some("passive"));
        assert_eq!(p.get("cable_vendor").copied(), Some("Apple"));
    }
}
