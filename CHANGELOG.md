# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.0] - 2026-06-10

### Changed (breaking — D-Bus interface)

**D-Bus interface bumped `org.usbeehive.Devices4` → `org.usbeehive.Devices5`,
hard cut, no alias.** Continues the 0.9.0 honest-units theme, this time for
the headline wattage itself. The kernel's UCSI driver derives `voltage_now`
and `current_now` from the negotiated PD contract (selected PDO voltage ×
RDO *operating current*) — they are never measured flow. A sink that is
e.g. battery-charge-limited at 80% may hold a healthy 65W contract while
requesting only 15W; presenting that 15W as "charging at 15W" reads as a
bad cable. The wire now separates the two numbers:

- **`power` tuple grows a field: `(uus)` → `(uuus)`** —
  `(power_in_mw, power_out_mw, contract_mw, power_role)`. `power_in_mw`
  keeps its position and fallback chain but is now documented as the
  sink's *requested* operating power (render as "up to N W"); the new
  `contract_mw` is what the active PDO contract *allows* (`0` = no
  contract inferred). `contract_mw > power_in_mw` ⟹ the sink is limiting
  its own draw. Positional unpackers must shift `power_role` from index 2
  to index 3. `ListDevices` becomes
  `a(ssssssssssqqsa(ss)ius(uuus)(bsssb)a(usuuuub)i)`.
- **New `bottleneck` variant `SinkLimit`** (always `is_warning == false`,
  so it never raises `CapabilityDegraded`): the contract is healthy but
  the sink requests < 80% of it — typically a battery charge limit or
  thermal policy. Distinguishes benign sink policy from the
  cable/negotiation bottlenecks the app exists to catch. Covered by the
  existing unknown-variant fallback rule for older clients.
- `DeviceLimit` / `Fine` diagnostic prose reworded to stop claiming
  measured flow: "Contract limited to NW" / "Charging at up to NW".

The CLI follows suit: `Charging in: 15W` → `Charging in: up to 15W (65W
contract)` when the contract allows more than the request, and `--json`
gains `power.contractMW`. The [usbee](https://github.com/abrauchli/usbee)
GNOME extension consumes the new wire as of its matching release and
requires `usbeehived` ≥ 0.10.0.

### Added

- **PD capabilities pair with ports via the partner's `usb_power_delivery`
  symlink.** Real kernels publish no `parent_port*` attribute on
  `/sys/class/usb_power_delivery/pdN` nodes — the partner directory's
  `usb_power_delivery` symlink (falling back to the `pdN` child directory)
  is the canonical linkage, and it now takes precedence in pairing. On
  multi-port machines the charger's full PDO list, `charger_max`, and the
  inferred active PDO finally surface on the right port. Includes a bugfix:
  `PowerDeliveryPort`'s derived `Default` left `parent_port_number` at `0`,
  spuriously pairing every unlinked PD node with Type-C port 0; it now
  defaults to `-1` ("not linked") as documented, and the sysfs reader
  parses the attribute when present.
- **No-e-marker 3A heuristic.** New `bottleneck` variant `CableNoEMarker`
  (`is_warning == true`, covered by the unknown-variant fallback rule):
  the sink's RDO current is pinned at 3A while the selected PDO offers
  more and no cable e-marker is visible — the signature of a non-e-marked
  cable, which the spec caps at 3A. Alongside it, a soft `properties` key
  `cable.no_emarker = "true"` fires whenever a charger advertises >3A but
  no e-marker rating is visible (phrased "not visible", never "missing" —
  some UCSI firmwares don't populate cable nodes at all). The CLI renders
  it as "No cable e-marker visible (3A limit may apply)". `DeviceLimit`
  detail now carries the same benign hint as `SinkLimit` ("often the
  device's own policy…") so a low contract doesn't read as a cable
  accusation.
- **Cable data-speed cross-check.** When a Hub/Peripheral partner's UFP
  VDO1 advertises a higher USB speed than the e-marked cable is rated
  for, a new `properties` key `cable.data_speed_limit` carries the
  cable's speed label (e.g. `"USB 2.0"`). The CLI renders it yellow as
  "Cable limits data to USB 2.0" — a slow cable on a fast device is the
  app's headline use case. New decoder `pd::decode_ufp_vdo_highest_speed`;
  `CableSpeed` now derives `Ord` (variants are locked in ascending speed
  order).

### Fixed

- **PDO values now parse on real kernels.** The kernel's typec pd class
  formats PDO attributes with unit suffixes (`5000mV`, `3000mA`, `45000mW`)
  and places a runtime-PM `power` directory inside `*-capabilities/`. The
  reader expected bare integers (the test-fixture convention), so on live
  hardware every PDO decoded as 0V/0A/0W plus one junk all-zero entry.
  Suffixes are now tolerated, the `power` directory is skipped, and the
  fixtures write kernel-style suffixed values. Likewise, real kernels
  publish no `type` attribute and no parseable index where the reader
  looked for them — both are encoded in the entry's directory name
  (`5:programmable_supply`), which now drives the fallback, so PPS
  profiles render their voltage range and `pdo_list[].index` is the
  spec's 1-based position instead of a constant 0.

### Internal

- `ChargingDiagnostic::evaluate` takes a third `requested_mw` argument
  (the RDO operating power; pass `0` when unavailable) and a fourth
  `requested_ma` (the RDO operating current in mA; `0` = unknown).
- `TypeCPowerSupply::negotiated_power_mw` and `PowerSummary` docs
  rewritten to state the negotiated-ceiling (not measurement) semantics;
  the misleading "live wattage is the ground truth" comment is gone.

## [0.9.0] - 2026-06-08

### Changed (breaking — D-Bus interface)

**D-Bus interface bumped `org.usbeehive.Devices3` → `org.usbeehive.Devices4`,
hard cut, no alias.** `BUS_NAME` (`org.usbeehive.Devices`), `OBJECT_PATH`
(`/org/usbeehive/Devices`), the 21-field per-entry signature, and every
method/signal are all unchanged. The break is confined to two `properties`
machine-key renames. Both keys were being read as live measurements when
each is in fact a *declared maximum*, so the names now say so:

- `usb_power_ma` → `usb_max_power_ma` — the USB `bMaxPower` descriptor draw
  ceiling (raw mA, 5 V assumed), not the instantaneous draw.
- `cable_current` → `cable_max_current` — the cable e-marker current
  *rating*, now parallel to its honest sibling `cable_max_power`.

The CLI text renderer relabels them to **"Max bus power (mA)"** and
**"Cable max current"** to match. The [usbee](https://github.com/abrauchli/usbee)
GNOME extension consumes the new keys as of its matching release and
requires `usbeehived` ≥ 0.9.0. A client that still queries the old key
strings simply stops finding those two rows — no value or signature change.

### Added

- **Pretty CLI labels for `transport.*` / `cable.trust.*` flag properties.**
  The boolean machine-key flags introduced by the 0.7.0 / 0.8.0 wire bumps
  (USB 2.0/3.x/4 link, DisplayPort / Thunderbolt 3 altmode, cable-trust
  zero-VID / unknown-VID / reserved-bits-set warnings) were falling through
  to their raw `key: true` form in the text renderer. A new
  `property_flag_label` plus a central `write_property` renderer now emit
  English labels, drop the trailing `: true` on flag keys, and color the
  `cable.trust.*` authenticity warnings YELLOW. Value-bearing and unknown
  keys keep their existing `Label: value` / raw passthrough, so daemon-side
  additions still render without a CLI change.

### Fixed

- **Live UCSI wattage is now reported even without a linked PD port.** On
  laptops where the kernel exposes no source-capabilities directory and the
  `pdN` nodes carry no `parent_port`, a real contract (e.g. a phone sourcing
  5V @ 3A) was dropped because the live reading was only consulted under
  `if let Some(pd_port)`. The port came out `power_in_mw=0`, got filtered
  out by usbee, and rendered "Powering 0.0 W". The live wattage is now read
  straight off `port.power_supply` and attributed by `power_role`,
  independent of PD-node linkage.
- **Friendlier headline when the iProduct descriptor is empty.** Built-in
  chips with no iProduct string (e.g. Intel's `8087:0029` Bluetooth radio)
  were headlined as the bare `VID:PID`. `DeviceSummary.headline` now falls
  back through `"<vendor> <class>"` → `"<class>"` → `"<vendor>"` →
  `"<vid>:<pid>"`. The topology tree view still uses the stable `vid:pid`
  identifier deliberately.

### Internal

- D-Bus integration tests for `transport.usb4` and active-PDO inference.
- Documented the `--features dbus` install path for the usbee GNOME extension.
- CI: force JavaScript actions onto Node.js 24; `cargo fmt` format-check fix.

## [0.8.0] - 2026-05-26

### Added

- **USB4 detection — `transport.usb4` machine-keyed property.** USB4 is
  not an altmode; it's negotiated at the USB-PD enter-mode layer and only
  surfaces through the `thunderbolt` subsystem. New
  `/sys/bus/thunderbolt/devices/` reader detects USB4-capable links and
  emits `transport.usb4=true` on Type-C ports with a partner when the
  system has both a USB4-capable host adapter (`generation >= 4`) and an
  attached USB4-capable router. TBT3-only docks under a USB4 host
  correctly stay silent.
- New `thunderbolt::ThunderboltRouter` plain-data type (`route`, `domain`,
  `is_host`, `generation`, vendor/device IDs and names, `unique_id`).
- New `Sysfs::thunderbolt_routers()` enumerator and
  `Sysfs::thunderbolt_dir()` helper.
- `Snapshot.thunderbolt_routers` field; `DeviceManager` exposes
  `thunderbolt_routers()` accessor.
- **Active PDO inference from live UCSI voltage.** `PowerDeliveryPort`
  gains `infer_active_source_pdo(live_mv)` which marks the source PDO
  matching the contracted voltage (fixed/battery within 500 mV
  tolerance; PPS/Variable by range). `summary::from_typec_port` invokes
  it when a UCSI psy is online and reports `voltage_now`, so the wire's
  `active_pdo_index` now resolves against real hardware instead of
  always returning `-1`.

### Changed (breaking — library API)

- `sysfs::manager::build_summaries` signature extended with a fourth
  `&[ThunderboltRouter]` argument carrying the Thunderbolt / USB4 router
  list. Pass `&[]` if you don't have the data.

D-Bus interface name is unchanged (`org.usbeehive.Devices3`). The two
new keys (`transport.usb4`, populated `active_pdo_index`) are additive
on the existing wire — clients that ignore unknown property keys are
unaffected.

## [0.7.0] - 2026-05-26

### Changed (breaking — D-Bus interface)

**D-Bus interface bumped `org.usbeehive.Devices2` → `org.usbeehive.Devices3`,
hard cut, no alias.** `BUS_NAME` (`org.usbeehive.Devices`) and
`OBJECT_PATH` (`/org/usbeehive/Devices`) unchanged. The wire gains a
structured PDO list and an active-PDO index. Three families of new
machine-keyed properties are added on top: cable trust signals,
active-transport flags. The GNOME extension port hadn't shipped against
0.6.0 yet, so this is a coordinated break-and-bump rather than a
deprecate-and-shim cycle.

#### Wire changes

- `ListDevices` element gains two trailing fields. Full per-entry signature
  becomes
  `a(ssssssssssqqsa(ss)ius(uus)(bsssb)a(usuuuub)i)`:
  - `pdo_list: a(usuuuub)` — `(index, kind, voltage_mv, max_voltage_mv,
    current_ma, power_mw, is_active)` per source PDO. Empty when the
    entry has no companion `PowerDeliveryPort`.
  - `active_pdo_index: i` — index into `pdo_list`, or `-1`.
- New `PdoEntry` wire type mirrors `crate::power::PowerDataObject`.
  `kind` is the label string (`"Fixed"` / `"Battery"` / `"Variable"` /
  `"PPS"` / `"Unknown"`).
- The legacy `charger_max` machine-key property is retained for back-compat;
  the structured `pdo_list` is the source of truth for UI rendering of
  charger PDO advertising.

#### New machine-keyed properties (a(ss) bag)

Pushed only when their flag fires — absence is the "off" state. Adding new
keys is non-breaking; the migration table is exhaustive for what 0.7.0
adds.

- `cable.trust.zero_vid=true` — cable ID Header VDO reports `vendor_id == 0`.
- `cable.trust.vid_unknown=true` — non-zero VID not in the bundled USB-IF
  vendor DB.
- `cable.trust.reserved_bits=true` — Cable VDO sets bits that USB-PD R3.x
  defines as reserved (bit 3 always; bits 7-8 for passive cables only).
- `transport.usb2=true` — UsbDevice with negotiated speed 1..=480 Mbps.
- `transport.usb3=true` — UsbDevice with negotiated speed ≥ 5000 Mbps.
- `transport.dp_altmode=true` — Type-C partner advertises SVID `0xFF01`
  (VESA DisplayPort).
- `transport.tb=true` — Type-C partner advertises SVID `0x8087`
  (Intel Thunderbolt 3).

#### Library type changes

- `cable::CableInfo` gains `trust: CableTrust { zero_vid, vid_unknown,
  reserved_bits_set }`. Populated once in `from_typec_cable`.
- `typec::TypeCPartner` gains `altmodes: Vec<TypeCAltMode>` populated from
  `port{N}-partner.{M}/` sibling altmode directories.
- `pd::cable_vdo_reserved_bits_set(vdo, is_active)` — new helper.
- `summary::is_phone()` signature changes to take an `&[UsbInterface]`
  slice — the new heuristic stack inspects interface descriptors.

### Fixed

- **Android phone misclassification (Cat S61 + others).** `is_phone()`
  previously only matched Apple+iphone or `product.contains("android")`,
  so any handset whose iProduct is the model name fell through to the
  inner CDC-ACM branch and got `DeviceClass::Serial` — surfaced as a
  terminal icon in the GNOME extension. New signal stack:
  1. ADB function signature `0xFF/0x42/0x01` (bullet-proof).
  2. PTP `0x06/0x01/0x01` paired with any vendor-class (`0xFF`) interface.
  3. `PHONE_VIDS` allowlist (Google, Samsung, Bullitt, Sony, OnePlus,
     Xiaomi, Huawei, ZTE, HTC, LG, MediaTek, Meizu, Oppo, Qualcomm)
     paired with PTP or vendor-class.
  Pure PTP-only DSLRs (Canon, Nikon) stay classified as Camera/Unknown.

## [0.6.0] - 2026-05-13

### Changed (breaking — D-Bus interface + lib API + CLI JSON output)

**D-Bus interface bumped `org.usbeehive.Devices1` → `org.usbeehive.Devices2`,
hard cut, no alias.** `BUS_NAME` (`org.usbeehive.Devices`) and
`OBJECT_PATH` (`/org/usbeehive/Devices`) unchanged. The interface
restructures the per-entry payload from prose `bullets` (the strings
displayed in `usbeehive`'s terminal output) into structured top-level
fields plus a `properties: a(ss)` bag of `(machine_key, value)` pairs.
The lib type [`DeviceSummary`] and the CLI's `--json` output mirror
the same structural shape — three surfaces, one source of truth.

The English display vocabulary for terminal rendering moves from
`src/summary.rs` (data layer) to `src/output.rs` (CLI renderer).
Clients consuming the wire / JSON receive machine keys only; they own
their translation table.

#### Wire / lib type changes

- `DeviceSummary.bullets: Vec<String>` is **gone**. Replaced by
  `DeviceSummary.properties: Vec<(String, String)>` with machine keys.
- New top-level fields on `DeviceSummary` and `DeviceEntry`:
  `device_class`, `device_subclass`, `link_speed_mbps`, `usb_version`,
  `power`, `vendor`, `product`, `vendor_id`, `product_id`,
  `primary_driver`.
- New `Status::Sourcing` variant — fires when the host is sourcing PD
  power out through a Type-C port (e.g. charging a phone).
- New `DeviceClass` enum: `Keyboard | Mouse | InputTablet | Gamepad |
  Storage | Display | Audio | Camera | VideoCapture | Printer | Phone
  | Hub | NetworkWired | NetworkWireless | SecurityKey |
  SmartcardReader | Bluetooth | Serial | Unknown`. `device_class` is
  `Unknown` for `Category::TypeCPort` (Type-C ports don't get a class).
- New `PowerSummary` / `PowerRole` types. `power_role` reflects the
  current contract direction (`Source` / `Sink`) when one exists,
  falling back to `DualRole` for dual-role-capable ports with no
  active contract, or `Unknown` for non-PD entries.
- `Diagnose(port)` return type is now `(present: b, bottleneck: s,
  summary: s, detail: s, is_warning: b)` — the leading `present` bool
  unambiguously distinguishes "no diagnostic computed" from
  "`bottleneck: Fine`".

#### Regex → structured field migration

For clients (USBee, dbus-monitor scripts) currently parsing prose
bullets:

| Today's regex / heuristic                            | Replaced by                          |
|---|---|
| `WATT_RE` (`/(\d+(?:\.\d+)?)\s*W\b/`)                | `power.power_in_mw` / `power_out_mw` |
| `DIRECTION_RE` (`/\b(sink\|source)\b/`)              | `power.power_role`                   |
| `USB_VERSION_RE` (`/\bUSB\s+\d…/`)                   | `usb_version`                        |
| `SPEED_RE` (`/(\d+(?:\.\d+)?)\s*(Gb\|Mb\|Kb)\/s/`)   | `link_speed_mbps`                    |
| substring scan `limited to` / `swap` / `degraded`    | `charging_diag.is_warning`           |
| per-bullet label-classifier regex                    | `properties[i].0` (machine key)      |
| headline substring → icon                            | `device_class` enum + `icon` string  |

#### Bullet text → property key migration

| Old bullet prose                       | New property key (`a(ss)` first column) |
|---|---|
| `Serial: ABC123`                       | `serial`                                |
| `Removable` / `Built-in`               | `mount` (value `removable` / `fixed`)   |
| `Drivers: usbhid, btusb`               | `drivers` (only when 2+ drivers); single driver moves to top-level `primary_driver` |
| `VID:PID 05ac:12a8`                    | top-level `vendor_id`, `product_id` (uint16) |
| `Data: host`                           | `data_role`                             |
| `Power mode: …`                        | `power_mode`                            |
| `PD revision: 3.0`                     | `pd_revision`                           |
| `Plug orientation: normal`             | `plug_orientation`                      |
| `Negotiated power: 20.0V @ 5.00A — 100W` | `pd_contract` (voltage/current); watts via `power.power_in_mw` |
| `Cable speed: USB 3.2 Gen 2`           | `cable_speed`                           |
| `Cable current: 5A`                    | `cable_current`                         |
| `Cable max power: 100W`                | `cable_max_power`                       |
| `Active cable` / `Passive cable`       | `cable_type` (value `active` / `passive`) |
| `Cable vendor: Realtek`                | `cable_vendor`                          |
| `Charger max: 100W`                    | `charger_max`                           |
| `Power: 500 mA` (USB bus-powered draw) | `usb_power_ma` (raw mA, voltage assumed 5 V) |

#### Enum extensibility convention

Adding a new variant to `DeviceClass` / `Status` / `PowerRole` /
`Bottleneck` (or any future enum on the wire) is a **non-breaking
change**. Clients MUST treat any unrecognized string as `Unknown` and
fall back to category-based behaviour. Removing or renaming variants
requires an interface bump.

Property keys follow the same rule — adding new keys is additive
(non-breaking); renaming or removing requires an interface bump.

#### `device_class` classification fidelity (day-one)

| Class               | Signal                                                          |
|---|---|
| `Keyboard` / `Mouse` | HID interface (class `0x03`) protocol `0x01` / `0x02`           |
| `Storage`           | Mass Storage interface (class `0x08`)                            |
| `Audio` / `Camera` / `VideoCapture` / `Printer` / `SmartcardReader` | matching interface base class |
| `Bluetooth`         | Wireless (`0xE0`) subclass `0x01` protocol `0x01`                |
| `NetworkWired` / `Serial` | CDC subclass (`0x06`/`0x02`) **or** known driver name allowlist |
| `Phone`             | Apple iPhone (VID + product); Android product string             |
| `SecurityKey`       | Yubico/Nitrokey VID, or product match (`yubikey`, `fido`, …)    |
| `Hub`               | `device_class == 0x09`                                          |
| `Gamepad` / `InputTablet` | HID + product-string heuristic                            |
| `Unknown`           | fallthrough — additive granularity later via new variants        |

#### USBee migration

The USBee GNOME extension will land its Devices2 port in lockstep
with this release. Replace the `Devices1` proxy interface name with
`Devices2` and drop every regex listed above in favor of the
structured fields. Property labels go through USBee's `_()` gettext —
the daemon-emitted machine keys are the binding layer, not the
display strings.

## [0.5.1] - 2026-05-09

Lint-only patch release. No behavior, API, or D-Bus interface changes.

### Fixed

- `cargo fmt --all`: re-flow long expression and chained iterator blocks
  that the original D-Bus daemon work landed unformatted.
- Replace 7× `&[x.clone()]` test-fixture argument patterns in
  `src/sysfs/manager.rs` with `std::slice::from_ref(&x)` to satisfy
  `clippy::cloned_ref_to_slice_refs` under `-D warnings`.
- Refactor `tests/dbus_interface.rs` so the crate-level `//!` doc
  comment is unconditional and only the dbus-dependent code is gated
  on `#[cfg(feature = "dbus")]`. Previously the entire file was
  cfg-gated, which stripped the doc comment when the feature was off
  and tripped `missing_docs` under CI's `cargo clippy --all-targets
  -- -D warnings` invocation.

## [0.5.0] - 2026-05-09

**Project renamed: `whatcable` → `usbeehive`.** At the request of the
original WhatCable author Darryl Morley, this Rust port no longer ships
under the `whatcable` name. The project, crate, CLI binary, daemon, and
D-Bus interface have all been renamed; thanks to Darryl for suggesting
the new name.

### Changed

- **Crate name:** `whatcable` → `usbeehive` (install with
  `cargo install usbeehive`). The previous `whatcable` crate is retired
  on crates.io with a redirect README; older versions are yanked.
- **CLI binary:** `whatcable` → `usbeehive`.
- **Daemon binary:** `whatcabled` → `usbeehived`.
- **D-Bus bus name / object path / interface:**
  `org.whatcable.Devices` → `org.usbeehive.Devices`,
  `/org/whatcable/Devices` → `/org/usbeehive/Devices`,
  `org.whatcable.Devices1` → `org.usbeehive.Devices1`. Existing D-Bus
  clients must update their proxy bindings.
- **Repository home:** [`abrauchli/whatcable`](https://github.com/abrauchli/whatcable)
  → [`abrauchli/usbeehive`](https://github.com/abrauchli/usbeehive). Full
  history was preserved in the move; the old repo is archived and points
  here.

### Added (carried over from unreleased)

- **Optional D-Bus interface (`dbus` feature).** New `usbeehive::dbus`
  module exposing `org.usbeehive.Devices1` (object path
  `/org/usbeehive/Devices`) with `ListDevices`, `ListPorts`, `Diagnose`,
  `SnapshotJson`, `Refresh` methods, `Version` / `DeviceCount`
  properties, and `DeviceAdded` / `DeviceRemoved` /
  `CapabilityDegraded` / `CapabilityRestored` signals. Disabled by
  default — pulls in `zbus` only when requested.
- **`usbeehived` daemon binary** (built with `--features dbus`). Runs
  the libudev hot-plug loop in a background thread and emits D-Bus
  signals as the snapshot changes. Suppresses signals on the initial
  baseline refresh so already-plugged devices don't re-fire as fresh
  events.
- **`Snapshot::diff(&Snapshot) -> SnapshotDiff`** — added/removed
  device ids plus newly-degraded / resolved Type-C port numbers. Backs
  the daemon's signal classification but is independently useful to
  library consumers.
- **`DeviceSummary::id() -> String`** — stable
  `"typec:<port_name>"` / `"usb:<bus_port>"` identifier, used as the
  diff key.
- `examples/dbus_client.rs` — minimal `zbus` client that lists devices
  and queries port 0's diagnostic.

### Migration from `whatcable` 0.4.0

```toml
# Cargo.toml — change the dependency name and version:
- whatcable = "0.4"
+ usbeehive = "0.5"
```

```rust
// Code — global rename:
- use whatcable::{DeviceManager, pd::*};
+ use usbeehive::{DeviceManager, pd::*};
```

```sh
# D-Bus clients — update bus name and interface:
- gdbus call --session --dest org.whatcable.Devices …
+ gdbus call --session --dest org.usbeehive.Devices …
```

The library API is otherwise identical to `whatcable` 0.4.0 plus the
D-Bus additions listed above; no breaking type / function changes
beyond the rename.

## [0.4.0] - 2026-05-08

Topology + link-speed surface: the USB tree view is now the default CLI
output, and the helpers that drive it are public on the library.

### Added

- **Tree topology view as the new default CLI output.** Renders the USB
  bus with `├─ └─` branches; each device is colored by its upstream link
  speed (gray → magenta, Low Speed through USB4) so bottlenecks are
  visible at a glance without per-line speed labels. Includes a legend
  at the bottom and italic styling for hubs. Empty root hubs are hidden;
  orphan devices (no enumerated parent, e.g. fixture-only) are still
  rendered as top-level roots.
- `--list` flag — restores the previous flat per-device view.
- `--tree` flag — explicit form of the new default.
- **`UsbDevice::parent_bus_port() -> Option<String>`** — sysfs
  identifier of the parent device, `None` for kernel root hubs.
- **`usb::tree_roots(&[UsbDevice]) -> Vec<&UsbDevice>`** — devices with
  no parent in the slice (root hubs + orphans), the entry points for a
  topology walk over `UsbDevice::children`.
- **`LinkSpeed` enum + `link_speed_tier(u32) -> LinkSpeed` +
  `UsbDevice::link_speed_tier()`** — a stable, switchable tier instead
  of raw Mbps thresholds. Adds a 40 Gbps USB4 tier the previous
  bucketing collapsed into "USB4 20 Gbps".

### Changed

- `speed_label(u32)` is now `link_speed_tier(speed).label()`. Existing
  labels (and their tests) are preserved verbatim.
- `src/sysfs/usb.rs::build_topology` and `src/output.rs` (tree renderer)
  drop their private parent-resolution / root-collection helpers in
  favor of the public API.

## [0.3.1] - 2026-05-02

Metadata-only patch release.

### Changed

- `repository` and `homepage` in `Cargo.toml` now point to
  `github.com/abrauchli/whatcable` (the active fork at the time;
  later renamed to `abrauchli/usbeehive`) instead of the upstream
  Zetaphor repo.
- MSRV correctly declared as `1.85` (was `1.74` in 0.3.0; the actual
  build hasn't worked on 1.74 since `clap_lex 1.1` adopted edition
  2024). Users running stable Rust are unaffected.

## [0.3.0] - 2026-05-02

Major rewrite: the crate gains a real library API behind feature flags,
a fixture-driven test suite, and several long-standing Type-C parsing
bugs are fixed. Same install path (`cargo install whatcable`), same
binary name (`whatcable`).

### Added

- **Feature-gated library API.** Three layers, each toggled by a Cargo feature:
  - **(default-off `cli`)** — the `whatcable` binary (clap + JSON / text rendering).
  - **(default-on `sysfs`)** — Linux `/sys` enumeration: `Sysfs` handle with
    injectable root, `DeviceManager`, `Snapshot`, `Error`, `Result`.
  - **(default-on `watch`)** — libudev hotplug: `watch::Watcher`,
    `watch::run_loop`, debounced render loop with `SIGINT` / `SIGTERM` handling.
  - Pure-decoder library use: `default-features = false`. Only `serde` is pulled.
- **`Sysfs::with_root(path)`** — injectable sysfs root for fixture-based testing
  and offline analysis. `Sysfs::try_with_root(path)` validates the path is a
  directory, returning `Error::InvalidRoot` otherwise.
- **`DeviceManager::with_sysfs(sysfs)`** + `Snapshot` struct — exposes the
  structured `usb_devices`, `typec_ports`, `pd_ports`, and `summaries`
  together so callers don't have to reach through accessor methods.
- **`TypeCPowerSupply::negotiated_power_mw()`** — `i128`-safe live wattage
  helper, computed from `voltage_now × current_now`. Used by both the
  human-readable bullet and the JSON `negotiatedPowerMW` field.
- **`--sysfs-root <PATH>`** CLI flag, useful for running the binary against
  captured fixture trees (and used by the new end-to-end smoke tests).
- Five runnable `examples/` — `decode_cable_vdo`, `cable_info`,
  `list_devices`, `snapshot_diff`, `print_changes`.
- 98 tests, up from ~30. Includes end-to-end CLI smoke tests that build a
  sysfs tree on disk and exercise the binary, plus integration tests
  covering charging diagnostics, PPS PDO parsing, deep USB topology, and
  cable-bottleneck detection.
- New CI workflow (`.github/workflows/ci.yml`): test matrix
  (default + `--no-default-features`), `cargo fmt --check`, `cargo clippy
  --all-targets -D warnings`, MSRV (1.85) build, rustdoc with `-D warnings`.
- Crate-level doc comments on every public item; runnable doctest in `lib.rs`.
- This `CHANGELOG.md`.

### Changed

- Library reorganisation. The previous flat `whatcable::*` re-exports remain
  the recommended entry point (`whatcable::DeviceManager`, `whatcable::pd::…`),
  but internal modules moved: sysfs IO is now under `whatcable::sysfs::*`.
- `ProductType` and `PdoType` use `#[derive(Default)]` + `#[default]` rather
  than hand-written `impl Default`.
- Workspace-wide rustfmt; clippy clean under `-D warnings`.

### Fixed

These four bugs existed in 0.2.1 but were only surfaced by the new
fixture-driven integration tests:

- **Type-C `read_identity` pushed VDOs in alphabetical filename order**
  (`cert_stat`, `id_header`, `product`, …) instead of USB-PD spec order
  (`id_header` first). Decoders treat `vdos[0]` as the ID Header and
  `vdos[3]` as the Cable VDO, so this silently mis-decoded every real
  cable / partner read from sysfs. Now walks a fixed `SPEC_ORDER` list,
  padding missing slots with `0` for stable indexing.
- **Type-C port enumeration accepted sibling entries** like
  `port0-partner` / `port0-cable` / `port0-plug0` as if they were ports,
  because the filter only checked `name.starts_with("port")` without
  requiring the trail to be numeric. Fix: reject any sibling whose
  trailing characters aren't all ASCII digits.
- **PPS PDO parsing used `voltage_mv = 0`** unless `maximum_voltage` was
  also missing. PPS / variable-supply PDOs publish `minimum_voltage` +
  `maximum_voltage` instead of a single `voltage` file; the parser now
  falls back to `minimum_voltage` whenever `voltage` itself is absent.
- **Deep USB topology dropped intermediate nodes**: `build_topology`
  cloned children from a pre-snapshot, so a `5-2.1.1` device ended up in
  the flat list, but its parent `5-2.1` was attached to `5-2.children`
  with empty `.children`. Fix: process devices deepest-first.

### Migration from 0.2.1

For most users — `cargo install whatcable` works exactly as before;
binary behavior is unchanged.

For library consumers:

- The previous `UsbDevice::enumerate()` / `TypeCPort::enumerate()` /
  `PowerDeliveryPort::enumerate()` (which read `/sys` directly) are gone.
  Use `whatcable::Sysfs::linux().usb_devices()` etc., or
  `DeviceManager::new()` for the bundled aggregate.
- `whatcable::manager::DeviceManager` moved to `whatcable::DeviceManager`
  (re-exported from the new `whatcable::sysfs::manager`).
- Want to drop the libudev / clap dependency tree? Set
  `default-features = false` and pick exactly the layer you need.

## [0.2.1] - 2025

- Switch CI to `ubuntu-latest` with apt.
- Cast signal handler through `*const ()` for clippy fn-to-int lint.
- Re-enable `watch` feature by default.

[0.9.0]: https://github.com/abrauchli/usbeehive/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/abrauchli/usbeehive/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/abrauchli/usbeehive/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/abrauchli/usbeehive/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/abrauchli/usbeehive/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/abrauchli/usbeehive/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/abrauchli/usbeehive/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/abrauchli/usbeehive/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/abrauchli/usbeehive/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/abrauchli/usbeehive/releases/tag/v0.2.1
