# WhatCable

> **What can this USB cable actually do?**

A command-line tool, and a set of Rust libraries, that tell you in plain
English what each USB device plugged into your Linux machine can actually do.

**WhatCable is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable),
a macOS menu-bar app by [Darryl Morley](https://github.com/darrylmorley).**
This port covers all USB devices, not just USB-C, while preserving the
rich USB-C Power Delivery diagnostics from the original.

The Rust rewrite is forked from
[Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux)
(originally C++ / CMake).

## Workspace layout

| Crate | Purpose |
|---|---|
| [`whatcable-core`](crates/whatcable-core) | Pure data types + USB-PD VDO decoders + diagnostics. No IO. |
| [`whatcable-sysfs`](crates/whatcable-sysfs) | Linux `/sys` backend with an injectable root. Returns `whatcable-core` types. |
| [`whatcable-watch`](crates/whatcable-watch) | libudev hotplug monitor; debounced render-loop helper. |
| [`whatcable`](crates/whatcable-cli) | The `whatcable` binary, with `--json` / `--watch` / `--raw` / `--sysfs-root`. |

If you want **the CLI**, install `whatcable`. If you want a **library**,
pick the smallest crate that covers your need:

- decoding USB-PD VDOs in isolation? Just `whatcable-core`.
- enumerating `/sys` and getting `DeviceSummary` lists? `whatcable-sysfs`
  (which re-exports `whatcable-core`).
- hotplug events? Add `whatcable-watch`.

## What it shows

### All USB devices
- **Identity**: vendor, product name, serial number
- **Speed**: negotiated link speed (1.5 Mbps to 20 Gbps)
- **USB version**: 1.1, 2.0, 3.0, 3.1, 3.2
- **Power draw**: how much power the device is consuming
- **Device type**: HID, Audio, Mass Storage, Hub, etc.
- **Driver**: which kernel driver is handling the device
- **Topology**: hub hierarchy showing what's plugged into what

### USB-C ports (additional detail)
- **Port roles**: data role (host/device), power role (source/sink)
- **Cable e-marker info**: cable speed capability, current rating (3A/5A), active vs passive, cable vendor
- **Charger PDO list**: every voltage/current profile the charger advertises, with the active profile highlighted
- **Charging diagnostics**: identifies bottlenecks — cable limiting speed, charger undersized, etc.
- **Partner identity**: decoded from PD Discover Identity VDOs

## Install

### From crates.io

```bash
cargo install whatcable
```

### Build from source

Requires Rust 1.74+ ([rustup](https://rustup.rs)) and `libudev` development
headers (for `--watch` hotplug support, on by default).

```bash
# Ubuntu / Debian
sudo apt install libudev-dev pkg-config

# Fedora
sudo dnf install systemd-devel pkgconf-pkg-config

# Arch / Manjaro
sudo pacman -S --needed systemd-libs pkgconf
```

```bash
cargo build --release                                                   # default (with --watch)
cargo build --release -p whatcable --no-default-features                # no libudev
sudo install -Dm755 target/release/whatcable /usr/local/bin/whatcable
```

### Tests

```bash
cargo test --workspace --no-default-features    # avoids libudev requirement
cargo test --workspace                          # full suite, requires libudev-dev
```

## CLI usage

```bash
whatcable                                  # human-readable summary of every USB device
whatcable --json                           # structured JSON output
whatcable --watch                          # stream updates as devices come and go
whatcable --raw                            # include raw sysfs attributes
whatcable --sysfs-root /tmp/fixture-root   # run against a captured tree (testing)
whatcable --version
whatcable --help
```

## Library usage

Decode a Cable VDO without any IO:

```rust
use whatcable_core::pd::{decode_cable_vdo, CableSpeed, CableCurrent};

let v = decode_cable_vdo(2 | (2 << 5) | (3 << 9), false);
assert_eq!(v.speed, CableSpeed::Usb32Gen2);
assert_eq!(v.current_rating, CableCurrent::FiveAmp);
assert_eq!(v.max_watts, 250);
```

Enumerate the system's USB tree:

```rust,no_run
use whatcable_sysfs::DeviceManager;

let mut mgr = DeviceManager::new();
mgr.refresh();
for s in mgr.devices() {
    println!("{}: {}", s.headline, s.subtitle);
}
```

Run a debounced render loop on hotplug events:

```rust,no_run
use std::time::Duration;
use whatcable_watch::run_loop;
use whatcable_sysfs::DeviceManager;

let mut mgr = DeviceManager::new();
run_loop(Duration::from_millis(500), |_reason| {
    mgr.refresh();
    println!("snapshot has {} devices", mgr.devices().len());
    Ok(())
}).unwrap();
```

See [`crates/*/examples/`](crates) for more.

## How it works

WhatCable reads three areas of the Linux sysfs filesystem. No root access
required for basic info:

| sysfs path | Provides |
|---|---|
| `/sys/bus/usb/devices/` | All USB devices: vendor, product, speed, power, class, interfaces, topology |
| `/sys/class/typec/` | USB-C port state: connection, roles, cable e-marker, partner identity |
| `/sys/class/usb_power_delivery/` | PD negotiation: PDO list from charger, active profile, PPS ranges |

Hotplug uses `libudev` to detect connect/disconnect events in real time.

Cable speed and power decoding follow the USB Power Delivery 3.x spec,
ported from the original WhatCable's Swift implementation.

## Caveats

- **USB-C/PD data availability varies by hardware.** The Type-C connector
  class and USB PD sysfs interfaces depend on the kernel driver
  (UCSI, TCPM, platform-specific). Some systems expose full PD negotiation
  data; others expose only basic port info or nothing at all.
- **Cable e-marker info only appears for cables that carry one.** Same as
  the original — most USB-C cables under 60W are unmarked.
- **WhatCable trusts the e-marker.** Counterfeit or mis-flashed cables can
  lie about their capabilities.
- **Vendor name lookup is not exhaustive.** Common vendors are recognized;
  others show the hex VID.

## Credits

Upstream Linux/KDE codebase: [Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux).

WhatCable is a port of [WhatCable](https://github.com/darrylmorley/whatcable)
by [Darryl Morley](https://github.com/darrylmorley). The USB Power Delivery
decoding logic, charging diagnostics, vendor database, and plain-English
summary approach are derived from the original macOS app.

## License

[MIT](LICENSE)
