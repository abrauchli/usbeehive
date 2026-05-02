//! Build a synthetic [`TypeCCable`] from raw VDOs and print the decoded
//! [`CableInfo`] — useful for verifying field interpretation against real
//! captures.
//!
//! ```sh
//! cargo run -p whatcable-core --example cable_info -- 0x205AC 0x000005AC 0 0x00000642
//! ```
//!
//! The four hex args are pushed in order as `id_header`, `cert_stat`,
//! `product`, `product_type_vdo1`. (For the average passive cable, only
//! the first and fourth are interesting.)

use std::collections::BTreeMap;

use whatcable_core::cable::CableInfo;
use whatcable_core::typec::{TypeCCable, TypeCIdentity};

fn main() {
    let args: Vec<u32> = std::env::args()
        .skip(1)
        .map(|s| {
            let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(&s).to_string();
            u32::from_str_radix(&s, 16).expect("hex u32")
        })
        .collect();
    if args.is_empty() {
        eprintln!("usage: cable_info <id_header> [cert_stat] [product] [product_type_vdo1]");
        std::process::exit(2);
    }

    let cable = TypeCCable {
        r#type: "passive".into(),
        plug_type: "type-c".into(),
        identity: Some(TypeCIdentity {
            vendor_id: (args[0] & 0xFFFF) as u16,
            product_id: 0,
            vdos: args.clone(),
        }),
        raw_attributes: BTreeMap::new(),
    };
    let info = CableInfo::from_typec_cable(&cable);

    println!("Vendor       : {} (0x{:04x})", info.vendor_name, info.vendor_id);
    println!("Type         : {}", info.cable_type);
    println!("Active       : {}", info.is_active);
    println!("Speed        : {:?}", info.speed);
    println!("Current      : {:?}", info.current_rating);
    println!("Max watts    : {}W", info.max_watts);
}
