# WhatCable-Linux — Agent Guidelines

## Project Overview

WhatCable-Linux is a Linux port of [WhatCable](https://github.com/darrylmorley/whatcable) (macOS) by Darryl Morley. It is a CLI tool that shows USB device and USB-C cable information by reading Linux sysfs.

This repository is a Rust rewrite of the original C++/CMake implementation.

## Architecture

A single Cargo crate exposing both a library (`whatcable_linux`) and a binary (`whatcable-linux`):

- **`src/lib.rs`** — re-exports the public API.
- **`src/main.rs`** — CLI entry. Argument parsing via `clap`.
- **`src/watch.rs`** — `--watch` mode loop (`poll(2)` + 500 ms debounce).
  Compiled only when the `watch` Cargo feature is enabled.

Hotplug uses **`libudev`** through the `udev` crate, gated behind the
`watch` Cargo feature (on by default). Disable with `--no-default-features`
to build/test on systems without libudev development headers.

## Key Data Flow

```
/sys/bus/usb/devices/         → src/usb.rs
/sys/class/typec/             → src/typec.rs
/sys/class/usb_power_delivery/ → src/power.rs
                                    ↓
                              src/manager.rs  ← src/monitor.rs (hotplug)
                                    ↓
                              src/summary.rs (plain-English output)
                                    ↓
                              src/output.rs → CLI (src/main.rs)
```

## Code Conventions

- Rust 2021, MSRV 1.74.
- All sysfs reads go through `crate::sysfs` — never read `/sys/` directly with
  raw `std::fs` from call sites.
- Handle missing sysfs paths gracefully — return `None` / empty collections,
  never panic. Many systems lack `/sys/class/typec/` or
  `/sys/class/usb_power_delivery/`.
- Source files derived from the original Swift code keep the attribution
  header noting the WhatCable / Zetaphor port lineage where applicable.
- Prefer `Option<T>` and `Result<T, E>` over sentinel values; prefer iterator
  chains over manual loops where the chain is clearer.
- Use `serde` derive for any type that may end up in `--json` output.

## Build

```bash
cargo build --release                          # default (with --watch)
cargo build --release --no-default-features    # without libudev / --watch
```

## Testing

```bash
cargo test --no-default-features    # no libudev required
cargo test                          # full, requires libudev-dev
```

Manual smoke tests:

- `./target/debug/whatcable-linux`
- `./target/debug/whatcable-linux --json`
- `./target/debug/whatcable-linux --watch`     (requires `watch` feature)

## Key Files to Know

| File | Purpose |
|---|---|
| `src/sysfs.rs` | Tiny helpers over `/sys` attribute reads — used by every module |
| `src/usb.rs` | Enumerates all USB devices from `/sys/bus/usb/devices/` |
| `src/typec.rs` | Reads USB-C port state from `/sys/class/typec/` |
| `src/pd.rs` | USB PD VDO bit-field decoding (ported from PDVDO.swift) |
| `src/power.rs` | Parses PDO lists from `/sys/class/usb_power_delivery/` |
| `src/cable.rs` | Decoded cable e-marker info |
| `src/diagnostic.rs` | Identifies USB-C charging bottlenecks |
| `src/summary.rs` | Generates headlines, subtitles, bullets per device |
| `src/manager.rs` | Aggregates all sources, correlates data, owns refresh logic |
| `src/monitor.rs` | libudev monitor + fd for `poll()` (feature-gated) |
| `src/watch.rs` | `--watch` event loop (binary-only, feature-gated) |
| `src/output.rs` | Text and JSON renderers |
| `src/vendor.rs` | USB VID → vendor name lookup |
| `src/usbclass.rs` | USB class code → human name |

## Adding New Vendors

Add entries to `lookup()` in `src/vendor.rs`. Format: `0xVID => "Vendor Name"`.

## Adding New USB Class Codes

Add cases to `class_name()` in `src/usbclass.rs`.

## Tests

Each module under `src/` carries its own `#[cfg(test)] mod tests`. Cover any
new bit-decoding, parsing, or label-formatting code with a small unit test
that does not depend on `/sys/` (use in-memory inputs or `tempdir` for sysfs
helpers).
