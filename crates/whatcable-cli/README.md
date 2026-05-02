# whatcable

Command-line interface for [WhatCable](https://github.com/Zetaphor/whatcable-linux).
Tells you what each USB cable / device on Linux can actually do.

```bash
cargo install whatcable
whatcable             # human-readable summary of every USB device
whatcable --json      # structured JSON output
whatcable --watch     # stream updates as devices come and go
whatcable --raw       # include raw sysfs attributes
whatcable --sysfs-root /path/to/captured/tree    # for fixture testing
```

## Build features

- `watch` (default) — enables `--watch` via `whatcable-watch`. Disable with
  `--no-default-features` to drop the `libudev` link dependency.

## Library crates

This package ships only the binary. For library use, depend on the
appropriate crate directly:

- [`whatcable-core`](https://crates.io/crates/whatcable-core) — pure types + USB-PD VDO decoders, no IO.
- [`whatcable-sysfs`](https://crates.io/crates/whatcable-sysfs) — Linux `/sys` enumeration backend.
- [`whatcable-watch`](https://crates.io/crates/whatcable-watch) — libudev hotplug monitor.

## License

[MIT](../../LICENSE)
