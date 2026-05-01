# WhatCable-Linux — Agent Guidelines

## Project Overview

WhatCable-Linux is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable) (macOS) by Darryl Morley. It is a CLI tool that shows USB device and USB-C cable information by reading Linux sysfs.

## Architecture

Two components share a single core library:

- **`src/core/`** — `libwhatcablecore`, a static C++ library (C++20, STL only). Reads sysfs over `std::filesystem` / streams, decodes USB PD data, and produces human-readable summaries. Links **`libudev`** for hotplug FD monitoring only — no Qt or other GUI/toolkit dependency.
- **`src/cli/`** — `whatcable-linux` CLI binary. POSIX `getopt_long`, manual JSON serialization for `--json`, `poll()` + 500 ms debounce for `--watch`.

## Key Data Flow

```
/sys/bus/usb/devices/         → UsbDevice.cpp
/sys/class/typec/             → TypeCPort.cpp
/sys/class/usb_power_delivery/ → PowerDelivery.cpp
                                    ↓
                              DeviceManager.cpp  ← UDevMonitor.cpp (hotplug)
                                    ↓
                              DeviceSummary.cpp (plain-English output)
                                    ↓
                              CLI (main.cpp)
```

## Code Conventions

- C++20, standard library. Use ordinary string literals or `std::string` / `std::string_view` as appropriate.
- All core classes are in the `WhatCable` namespace.
- sysfs reads go through `SysfsReader` — never read `/sys/` directly with raw file I/O from call sites (implementation may use filesystem APIs inside `SysfsReader`).
- Source files derived from the original Swift code must keep the attribution header: `// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)`
- Handle missing sysfs paths gracefully — return empty/nullopt, never crash. Many systems lack `/sys/class/typec/` or `/sys/class/usb_power_delivery/`.

## Build

```bash
cmake -B build
cmake --build build
```

## Testing

- Run the CLI: `./build/src/cli/whatcable-linux`
- JSON output: `./build/src/cli/whatcable-linux --json`
- Watch mode: `./build/src/cli/whatcable-linux --watch`

## Key Files to Know

| File | Purpose |
|---|---|
| `src/core/UsbDevice.h/cpp` | Enumerates all USB devices from `/sys/bus/usb/devices/` |
| `src/core/TypeCPort.h/cpp` | Reads USB-C port state from `/sys/class/typec/` |
| `src/core/PDDecoder.h/cpp` | USB PD VDO bit-field decoding (ported from PDVDO.swift) |
| `src/core/PowerDelivery.h/cpp` | Parses PDO lists from `/sys/class/usb_power_delivery/` |
| `src/core/DeviceSummary.h/cpp` | Generates headlines, subtitles, bullets per device |
| `src/core/ChargingDiagnostic.h/cpp` | Identifies USB-C charging bottlenecks |
| `src/core/DeviceManager.h/cpp` | Aggregates all sources, correlates data, owns refresh logic |
| `src/core/UDevMonitor.h/cpp` | libudev monitor + fd for `poll()` |
| `src/core/VendorDB.h/cpp` | USB VID → vendor name lookup |
| `src/core/UsbClassDB.h/cpp` | USB class code → human name |

## Adding New Vendors

Add entries to the `kVendors` map in `src/core/VendorDB.cpp`. Format: `{0xVID, "Vendor Name"}`.

## Adding New USB Class Codes

Add cases to `UsbClassDB::className()` or `interfaceClassName()` in `src/core/UsbClassDB.cpp`.
