//! USB base-class code → human-readable label.
//!
//! Mapping follows the USB-IF "Defined Class Codes" registry. Unknown codes
//! return their hex representation (`"0xaa"`) so log output stays readable.

/// Return a label for the USB base class `code`. Unknown values fall back to
/// a `"0x..."` hex string.
///
/// ```
/// use whatcable::usbclass::class_name;
/// assert_eq!(class_name(0x09), "Hub");
/// assert_eq!(class_name(0xAA), "0xaa");
/// ```
pub fn class_name(code: u8) -> String {
    match code {
        0x00 => "Composite",
        0x01 => "Audio",
        0x02 => "Communications",
        0x03 => "HID",
        0x05 => "Physical",
        0x06 => "Image",
        0x07 => "Printer",
        0x08 => "Mass Storage",
        0x09 => "Hub",
        0x0A => "CDC Data",
        0x0B => "Smart Card",
        0x0D => "Content Security",
        0x0E => "Video",
        0x0F => "Personal Healthcare",
        0x10 => "Audio/Video",
        0x11 => "Billboard",
        0x12 => "USB Type-C Bridge",
        0xDC => "Diagnostic",
        0xE0 => "Wireless",
        0xEF => "Miscellaneous",
        0xFE => "Application Specific",
        0xFF => "Vendor Specific",
        _ => return format!("0x{code:02x}"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_classes() {
        assert_eq!(class_name(0x09), "Hub");
        assert_eq!(class_name(0x03), "HID");
        assert_eq!(class_name(0x08), "Mass Storage");
    }

    #[test]
    fn unknown_class_is_hex() {
        assert_eq!(class_name(0xAA), "0xaa");
        assert_eq!(class_name(0x42), "0x42");
    }
}
