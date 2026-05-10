//! Decode a hex Cable VDO from the command line and print the parsed view.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p usbeehive-core --example decode_cable_vdo -- 0x00000642
//! ```

use std::process::ExitCode;

use usbeehive::pd::{
    cable_current_label, cable_speed_label, decode_cable_vdo, decode_id_header, product_type_label,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: decode_cable_vdo <hex-id-header> [hex-cable-vdo]");
        return ExitCode::from(2);
    }

    let id_hex = parse_hex(&args[0]);
    let id_hdr = decode_id_header(id_hex);
    println!("ID Header VDO: 0x{id_hex:08x}");
    println!("  vendor_id    = 0x{:04x}", id_hdr.vendor_id);
    if let Some(p) = id_hdr.ufp_product_type {
        println!(
            "  ufp_product  = {} ({})",
            product_type_label(p),
            p_label(p)
        );
    }
    if let Some(p) = id_hdr.dfp_product_type {
        println!(
            "  dfp_product  = {} ({})",
            product_type_label(p),
            p_label(p)
        );
    }
    println!("  comm host    = {}", id_hdr.usb_comm_capable_as_host);
    println!("  comm device  = {}", id_hdr.usb_comm_capable_as_device);
    println!("  modal_op     = {}", id_hdr.modal_operation);

    if let Some(cable_arg) = args.get(1) {
        let cable_hex = parse_hex(cable_arg);
        let cable = decode_cable_vdo(cable_hex, /* is_active */ false);
        println!();
        println!("Cable VDO: 0x{cable_hex:08x}");
        println!("  speed        = {}", cable_speed_label(cable.speed));
        println!(
            "  current      = {}",
            cable_current_label(cable.current_rating)
        );
        println!("  max VBUS     = {}V", cable.max_vbus_volts);
        println!("  max watts    = {}W", cable.max_watts);
        println!("  vbus thru    = {}", cable.vbus_through_cable);
    }

    ExitCode::SUCCESS
}

fn parse_hex(s: &str) -> u32 {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u32::from_str_radix(s, 16).unwrap_or_else(|_| {
        eprintln!("not a hex u32: {s}");
        std::process::exit(2);
    })
}

fn p_label(p: usbeehive::pd::ProductType) -> &'static str {
    use usbeehive::pd::ProductType::*;
    match p {
        Hub => "hub",
        Peripheral => "peripheral",
        PassiveCable => "passive cable",
        ActiveCable => "active cable",
        Ama => "AMA",
        Vpd => "VPD",
        Other => "other",
        Undefined => "undefined",
    }
}
