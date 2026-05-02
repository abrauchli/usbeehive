//! USB Implementers Forum vendor ID lookup.
//!
//! A small static table covering vendors most commonly seen in a typical
//! Linux USB tree. Unknown VIDs return a hex fallback string ([`is_hex_fallback`]
//! identifies these so callers can suppress vendor labelling for them).

/// Look up a USB Vendor ID. Returns the canonical vendor name, or a
/// `"0x...."` hex string if the VID isn't in the bundled table.
///
/// ```
/// use whatcable::vendor::lookup;
/// assert_eq!(lookup(0x05AC), "Apple");
/// assert_eq!(lookup(0xDEAD), "0xdead");
/// ```
pub fn lookup(vid: u16) -> String {
    match vid {
        0x05AC => "Apple",
        0x0B95 => "ASIX Electronics",
        0x2109 => "VIA Labs",
        0x0BDA => "Realtek",
        0x8087 => "Intel",
        0x04B4 => "Cypress",
        0x1D6B => "Linux Foundation",
        0x1A40 => "Terminus Technology",
        0x2188 => "CalDigit",
        0x17EF => "Lenovo",
        0x04E8 => "Samsung",
        0x0409 => "NEC",
        0x0835 => "Action Star Enterprise",
        0x291A => "Anker",
        0x1BCF => "Sunplus",
        0x2D01 | 0x18D1 => "Google",
        0x046D => "Logitech",
        0x1532 => "Razer",
        0x1B1C => "Corsair",
        0x258A => "Hailuck",
        0x0951 => "Kingston",
        0x090C => "Silicon Motion",
        0x0781 => "SanDisk",
        0x0930 => "Toshiba",
        0x13FE => "Phison",
        0x058F => "Alcor Micro",
        0x05E3 => "Genesys Logic",
        0x0451 => "Texas Instruments",
        0x1058 => "Western Digital",
        0x0BC2 => "Seagate",
        0x174C | 0x1B21 => "ASMedia",
        0x0438 | 0x1022 => "AMD",
        0x0955 => "NVIDIA",
        0x056A => "Wacom",
        0x1050 => "Yubico",
        0x045E => "Microsoft",
        0x03F0 => "HP",
        0x413C => "Dell",
        0x04F2 => "Chicony",
        0x0C45 => "Microdia",
        0x0BDB => "Ericsson",
        0x2C7C => "Quectel",
        0x1199 => "Sierra Wireless",
        0x12D1 => "Huawei",
        0x22B8 => "Motorola",
        0x0E8D => "MediaTek",
        0x0763 => "M-Audio",
        0x1235 => "Focusrite",
        0x1397 => "BEHRINGER",
        0x0582 => "Roland",
        0x1043 => "iCreate Technologies",
        0x262A => "Savitech",
        0x37A8 => "Ultimate Gadget Laboratories",
        _ => return format!("0x{vid:04x}"),
    }
    .to_string()
}

/// `true` when [`lookup`]'s output is the synthetic hex fallback rather than
/// a real vendor name.
pub fn is_hex_fallback(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vendors() {
        assert_eq!(lookup(0x05AC), "Apple");
        assert_eq!(lookup(0x18D1), "Google");
        assert_eq!(lookup(0x2D01), "Google");
    }

    #[test]
    fn unknown_falls_back_to_hex() {
        let s = lookup(0xDEAD);
        assert_eq!(s, "0xdead");
        assert!(is_hex_fallback(&s));
    }

    #[test]
    fn real_name_is_not_hex_fallback() {
        assert!(!is_hex_fallback("Apple"));
        assert!(!is_hex_fallback(""));
    }
}
