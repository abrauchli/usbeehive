# WhatCable

> **What can this USB cable actually do?**

A command-line tool that tells you, in plain English, what each USB device plugged into your Linux machine can actually do.

**WhatCable is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable), a macOS menu bar app by [Darryl Morley](https://github.com/darrylmorley).** This port expands the original USB-C focus to cover all USB devices, while preserving the rich USB-C Power Delivery diagnostics from the original.

This repository is a Rust rewrite forked from [Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux) (originally C++/CMake).

## What it shows

### All USB devices
- **Device identity**: vendor, product name, serial number
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

### Build from source

Requires Rust 1.74+ (install via [rustup](https://rustup.rs)) and `libudev`
development headers (used for `--watch` hotplug support, enabled by default).

```bash
# Ubuntu / Debian
sudo apt install libudev-dev pkg-config

# Fedora
sudo dnf install systemd-devel pkgconf-pkg-config

# Arch / Manjaro
sudo pacman -S --needed systemd-libs pkgconf
```

Build:

```bash
cargo build --release                              # default (with --watch)
cargo build --release --no-default-features        # without libudev / --watch
sudo install -Dm755 target/release/whatcable-linux /usr/local/bin/whatcable-linux
```

### Tests

```bash
cargo test --no-default-features    # avoids libudev requirement
cargo test                          # full suite, requires libudev-dev
```

### Usage

```bash
whatcable-linux              # human-readable summary of every USB device
whatcable-linux --json       # structured JSON output
whatcable-linux --watch      # stream updates as devices come and go
whatcable-linux --raw        # include raw sysfs attributes
whatcable-linux --version
whatcable-linux --help
```

## How it works

WhatCable reads three areas of the Linux sysfs virtual filesystem. No root access required for basic info:

| sysfs path | What it gives us |
|---|---|
| `/sys/bus/usb/devices/` | All USB devices: vendor, product, speed, power, class, interfaces, topology |
| `/sys/class/typec/` | USB-C port state: connection, roles, cable e-marker, partner identity |
| `/sys/class/usb_power_delivery/` | PD negotiation: PDO list from charger, active profile, PPS ranges |

Hotplug monitoring uses `libudev` to detect connect/disconnect events in real time.

Cable speed and power decoding follow the USB Power Delivery 3.x spec, ported from the original WhatCable's Swift implementation.

## Caveats

- **USB-C/PD data availability varies by hardware.** The Type-C connector class and USB PD sysfs interfaces depend on the kernel driver (UCSI, TCPM, platform-specific). Some systems expose full PD negotiation data; others expose only basic port info or nothing at all.
- **Cable e-marker info only appears for cables that carry one.** Same as the original — most USB-C cables under 60W are unmarked.
- **WhatCable trusts the e-marker.** Counterfeit or mis-flashed cables can lie about their capabilities.
- **Vendor name lookup is not exhaustive.** Common vendors are recognized; others show the hex VID.

## Credits

Upstream Linux/KDE codebase: [Zetaphor/whatcable-linux](https://github.com/Zetaphor/whatcable-linux).

WhatCable is a port of [WhatCable](https://github.com/darrylmorley/whatcable) by [Darryl Morley](https://github.com/darrylmorley). The USB Power Delivery decoding logic, charging diagnostics, vendor database, and plain-English summary approach are derived from the original macOS app.

## License

[MIT](LICENSE)
