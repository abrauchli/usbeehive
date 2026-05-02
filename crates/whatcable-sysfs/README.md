# whatcable-sysfs

Linux sysfs backend for [WhatCable](https://github.com/Zetaphor/whatcable-linux).
Walks `/sys/bus/usb/devices`, `/sys/class/typec`, and
`/sys/class/usb_power_delivery` and returns the structured types from
[`whatcable-core`](https://crates.io/crates/whatcable-core).

## Injectable root

The root path is supplied at construction time, so tests can point the
enumerator at a fixture tree on disk.

```rust
use whatcable_sysfs::{DeviceManager, Sysfs};

let mut mgr = DeviceManager::with_sysfs(Sysfs::with_root("/path/to/fixture"));
mgr.refresh();
println!("{} devices", mgr.devices().len());
```

In production, [`Sysfs::linux`] is the default and binds to `/sys`.

## Errors

Enumeration is intentionally infallible. Missing files yield empty results
because that's the natural sysfs idiom — kernel attributes come and go as
drivers load and detach. The single fallible API is
[`Sysfs::try_with_root`], which validates that the supplied path exists.

## Tests

The crate ships with a programmatic fixture builder (see
`tests/fixture_builder.rs`) that mirrors representative kernel layouts:

- USB tree with hubs and a multi-level topology
- Type-C port with a 5A active cable, PPS-capable charger
- Cable-bottleneck scenario flagging the diagnostic warning

## License

[MIT](../../LICENSE)
