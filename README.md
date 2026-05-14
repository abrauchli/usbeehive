# usbeehive 🐝

> **What can this USB cable actually do?**

A command-line tool, and a Rust library, that tell you in plain English
what each USB device plugged into your Linux machine can actually do.

**usbeehive is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable),
a macOS menu-bar app by [Darryl Morley](https://github.com/darrylmorley).**
This port covers all USB devices, not just USB-C, while preserving the
rich USB-C Power Delivery diagnostics from the original.

The Rust rewrite is forked from
[Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux)
(originally C++ / CMake).

> **Renamed from `whatcable`.** This project was previously published on
> crates.io as [`whatcable`](https://crates.io/crates/whatcable) — at the
> original author's request, it has been renamed to `usbeehive` as of
> 0.5.0. The old crate is retired (its final release is a redirect to
> here). Use `cargo install usbeehive` going forward.

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
- **Charger PDO list**: every voltage / current profile the charger advertises, with the active profile highlighted
- **Charging diagnostics**: identifies bottlenecks — cable limiting speed, charger undersized, etc.
- **Live wattage** (UCSI): `voltage_now × current_now`, exposed as `negotiatedPowerMW` in JSON output and via `TypeCPowerSupply::negotiated_power_mw()` in the library.
- **Partner identity**: decoded from PD Discover Identity VDOs

## Install

### From crates.io

```bash
cargo install usbeehive
```

### Build from source

Requires Rust 1.85+ ([rustup](https://rustup.rs)) and `libudev` development
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
cargo build --release                                  # default (with --watch)
cargo build --release --no-default-features --features cli,sysfs    # no libudev
sudo install -Dm755 target/release/usbeehive /usr/local/bin/usbeehive
```

### Tests

```bash
cargo test                                # full suite (requires libudev-dev)
cargo test --no-default-features          # pure-decoder subset, no libudev
```

## CLI usage

```bash
usbeehive                                  # human-readable summary of every USB device
usbeehive --json                           # structured JSON output
usbeehive --watch                          # stream updates as devices come and go
usbeehive --raw                            # include raw sysfs attributes
usbeehive --sysfs-root /tmp/fixture-root   # run against a captured tree (testing)
usbeehive --version
usbeehive --help
```

## Library usage

The crate has three optional layers, each behind a Cargo feature so
consumers pull in only what they need.

| Feature | Default | Adds |
|---|---|---|
| (none) | always | Data types, USB-PD VDO decoders, diagnostics, summaries — IO-free. |
| `sysfs` | yes | `Sysfs` handle + `DeviceManager` — Linux `/sys` enumeration with injectable root. |
| `watch` | yes | libudev hotplug monitor: `watch::Watcher` + `watch::run_loop`. |
| `cli` | yes | The `usbeehive` binary (clap + JSON / text rendering). |
| `dbus` | no | `usbeehive::dbus` interface module + the `usbeehived` daemon binary publishing `org.usbeehive.Devices2` on the session bus (implies `watch`). |

Library-only consumers can drop the binary deps:

```toml
# Pure decoders, no IO, no libudev:
usbeehive = { version = "0.5", default-features = false }

# Add /sys enumeration:
usbeehive = { version = "0.5", default-features = false, features = ["sysfs"] }

# Add hotplug too:
usbeehive = { version = "0.5", default-features = false, features = ["watch"] }
```

### Decode a Cable VDO

```rust
use usbeehive::pd::{decode_cable_vdo, CableSpeed, CableCurrent};

let v = decode_cable_vdo(2 | (2 << 5) | (3 << 9), false);
assert_eq!(v.speed, CableSpeed::Usb32Gen2);
assert_eq!(v.current_rating, CableCurrent::FiveAmp);
assert_eq!(v.max_watts, 250);
```

### Enumerate the system's USB tree

```rust,no_run
use usbeehive::DeviceManager;

let mut mgr = DeviceManager::new();
mgr.refresh();
for s in mgr.devices() {
    println!("{}: {}", s.headline, s.subtitle);
}
```

### Run a debounced render loop on hotplug events

```rust,no_run
use std::time::Duration;
use usbeehive::{watch::run_loop, DeviceManager};

let mut mgr = DeviceManager::new();
run_loop(Duration::from_millis(500), |_reason| {
    mgr.refresh();
    println!("snapshot has {} devices", mgr.devices().len());
    Ok(())
}).unwrap();
```

See [`examples/`](examples) for more.

## D-Bus daemon (optional)

Build with the `dbus` feature to get a long-running daemon, `usbeehived`,
that publishes the live snapshot on the session bus. Useful for desktop
applets (KDE / GNOME / tray apps) that want to react to cable plugs and
charging-bottleneck changes without each one re-implementing sysfs
enumeration.

```bash
cargo build --release --no-default-features --features dbus
./target/release/usbeehived                                # foreground
```

Wire surface — `org.usbeehive.Devices2` at `/org/usbeehive/Devices`:

Each `ListDevices` entry is a 19-field tuple:
`(id, category, device_class, device_subclass, status, headline, subtitle, icon, vendor, product, vendor_id, product_id, primary_driver, properties, port_number, link_speed_mbps, usb_version, power, charging_diag)`. See [`src/dbus.rs`](src/dbus.rs) module docs for per-field semantics.

| Member | Kind | Notes |
|---|---|---|
| `ListDevices` | method `() → a(ssssssssssqqsa(ss)ius(uus)(bsssb))` | One structured entry per summary. Properties are `(machine_key, value)` pairs. |
| `ListPorts` | method `() → ai` | Type-C `port_number`s currently exposed. |
| `Diagnose` | method `(i) → (bsssb)` | Charging diagnostic for a port — same shape as the per-entry `charging_diag`. `present == false` when none available. |
| `SnapshotJson` | method `() → s` | Full structured snapshot — same shape as `usbeehive --json`. |
| `Refresh` | method `() → u` | Force a re-enumeration; returns the new device count. |
| `Version` / `DeviceCount` | properties | Crate version + summary count. |
| `DeviceAdded` / `DeviceRemoved` | signals | Fire when a device or port appears / disappears. |
| `CapabilityDegraded` / `CapabilityRestored` | signals | Fire when a port's charging diagnostic newly raises (or clears) `is_warning` — e.g. a too-thin cable plugged into a beefy charger. |

Quick poke from the shell:

```sh
gdbus call --session --dest org.usbeehive.Devices \
    --object-path /org/usbeehive/Devices \
    --method org.usbeehive.Devices2.ListDevices
```

A minimal Rust client lives in [`examples/dbus_client.rs`](examples/dbus_client.rs).

### Devices2 migration

`Devices1` was retired in favor of `Devices2` — clients must update both
the interface name and how they consume the per-entry payload. Adding
new enum variants (`device_class`, `status`, `power_role`, `bottleneck`)
is non-breaking; clients MUST treat any unrecognized string as
`Unknown` / fall back to category-based behavior. The CHANGELOG has the
full regex → field migration table.

## How it works

usbeehive reads three areas of the Linux sysfs filesystem. No root access
required for basic info:

| sysfs path | Provides |
|---|---|
| `/sys/bus/usb/devices/` | All USB devices: vendor, product, speed, power, class, interfaces, topology |
| `/sys/class/typec/` | USB-C port state: connection, roles, cable e-marker, partner identity |
| `/sys/class/usb_power_delivery/` | PD negotiation: PDO list from charger, active profile, PPS ranges |
| `/sys/class/power_supply/ucsi-source-psy-*` | Live `voltage_now × current_now` charging readout |

Hotplug uses `libudev` to detect connect/disconnect events in real time.

Cable speed and power decoding follow the USB Power Delivery 3.x spec,
ported from the original WhatCable's Swift implementation.

## Caveats

- **USB-C / PD data availability varies by hardware.** The Type-C connector
  class and USB PD sysfs interfaces depend on the kernel driver
  (UCSI, TCPM, platform-specific). Some systems expose full PD negotiation
  data; others expose only basic port info or nothing at all.
- **Cable e-marker info only appears for cables that carry one.** Same as
  the original — most USB-C cables under 60W are unmarked.
- **usbeehive trusts the e-marker.** Counterfeit or mis-flashed cables can
  lie about their capabilities.
- **Vendor name lookup is not exhaustive.** Common vendors are recognized;
  others show the hex VID.

## Credits

Original macOS app: [WhatCable](https://github.com/darrylmorley/whatcable)
by [Darryl Morley](https://github.com/darrylmorley) — and thanks to Darryl
for suggesting the new name when the Rust port outgrew its borrowed one.
The USB Power Delivery decoding logic, charging diagnostics, vendor
database, and plain-English summary approach are derived from the original
macOS app.

Upstream Linux/KDE C++ port: [Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux).

## License

[MIT](LICENSE)
