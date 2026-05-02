# whatcable-core

Pure-data types and decoders for USB, USB Type-C, and USB Power Delivery —
the IO-free heart of [WhatCable](https://github.com/Zetaphor/whatcable-linux).

This crate contains no IO. It provides:

- **Snapshot types** — `UsbDevice`, `TypeCPort`, `PowerDeliveryPort`,
  `CableInfo`, `DeviceSummary` — all `serde::Serialize`-able.
- **USB-PD 3.x VDO decoders** — `pd::decode_id_header`, `pd::decode_cable_vdo`.
- **USB class / vendor lookups** — `usbclass::class_name`, `vendor::lookup`.
- **Diagnostics** — `ChargingDiagnostic::evaluate` classifies why a USB-C
  port is not charging at full speed.

For a Linux sysfs backend that produces these types, depend on
[`whatcable-sysfs`](https://crates.io/crates/whatcable-sysfs). For hotplug
monitoring, see [`whatcable-watch`](https://crates.io/crates/whatcable-watch).

## Example

```rust
use whatcable_core::pd::{decode_cable_vdo, CableSpeed, CableCurrent};

let raw: u32 = 2 | (2 << 5) | (3 << 9); // Gen2, 5A, 50V
let v = decode_cable_vdo(raw, false);
assert_eq!(v.speed, CableSpeed::Usb32Gen2);
assert_eq!(v.current_rating, CableCurrent::FiveAmp);
assert_eq!(v.max_watts, 250);
```

## License

[MIT](../../LICENSE)
