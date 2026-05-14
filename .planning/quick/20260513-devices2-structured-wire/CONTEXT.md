---
slug: devices2-structured-wire
date: 2026-05-13
status: locked
---

# Devices2 structured wire — Context (locked)

**Gathered:** 2026-05-13
**Locked:** 2026-05-13 (after two rounds with USBee maintainers)
**Status:** Ready for execution — no open questions

<domain>
## Phase boundary

Bump the D-Bus interface from `org.usbeehive.Devices1` to
`org.usbeehive.Devices2` with a structured per-device payload, plus
mirror the same structural shape in the lib (`DeviceSummary`) and the
CLI's `--json` output. Eliminate the regex-prose round-trip USBee
currently does.

`BUS_NAME` (`org.usbeehive.Devices`) and `OBJECT_PATH`
(`/org/usbeehive/Devices`) unchanged. Hard cut — `Devices1` removed.

</domain>

<decisions>
## Locked decisions

### Scope — single Devices2 bump

All six structural fields ship together: `device_class` +
`properties` + `link_speed_mbps` + `usb_version` + `power` +
`charging_diag`. Plus the USBee-asked additions: `device_subclass`,
`primary_driver`, `vendor`, `product`, `vendor_id (q)`, `product_id (q)`,
`Status::Sourcing`. One USBee migration, one release, one wire
contract.

### Three surfaces, one shape

This is a unification phase, not just a wire refactor:

1. **`DeviceSummary` (lib)** — drops `bullets: Vec<String>`, gains
   `properties: Vec<(String, String)>` with machine keys plus every
   structured field listed below.
2. **`usbeehive --json` (CLI)** — replaces `bullets` with
   `properties`; exposes every new top-level field. Breaking change to
   the JSON consumer surface, documented in CHANGELOG.
3. **`org.usbeehive.Devices2` (D-Bus)** — the same shape over the
   wire.

Owner of English display prose moves from `src/summary.rs` to
`src/output.rs` (the CLI text renderer). Daemon never emits
human-readable English on the wire; CLI owns the key → English label
map for terminal rendering.

### v1 lifetime — hard cut

`Devices1` removed in same commit. Daemon registers only Devices2.
No alias. Pre-wide-adoption — USBee migrates in lockstep.

### Enum serialization — strings

Every enum across the wire is the Rust `Debug` variant name as UTF-8.
Applies to `category`, `device_class`, `status`, `power_role`,
`bottleneck`.

### Enum extensibility — non-breaking additions

Adding a new variant to `device_class` / `device_subclass` / `status`
/ `power_role` / `bottleneck` is **non-breaking**. UIs MUST treat any
unrecognized string as `Unknown` (or fallback to category-based
behavior). Documented in CHANGELOG and module docs.

USBee will add a test asserting unknown-variant fallthrough.

### `charging_diag` shape — `(bsssb)` everywhere

`(present: b, bottleneck: s, summary: s, detail: s, is_warning: b)`.
Empty string is **not** the absence sentinel — `present: bool` is.
Same struct on per-entry `charging_diag` and on `Diagnose(port)`.
"Fine" is a non-empty bottleneck; the `present` bit unambiguously
separates "no diagnostic computed" from "Fine charging".

`charging_diag.present` is false on every non-`Charging` status (the
bottleneck framing only evaluates against a sink contract). On
`Sourcing`, `Connected`, `Empty` → always absent.

### `power_role` semantics — flow when contracted, capability when not

| Port state | `status` | `power_role` |
|---|---|---|
| Empty TypeC                            | `Empty`     | `Unknown`  |
| TypeC connected, dual-role-capable, no contract | `Connected` | `DualRole` |
| TypeC connected, source-only           | `Connected` | `Source`   |
| TypeC connected, sink-only             | `Connected` | `Sink`     |
| TypeC actively sinking                 | `Charging`  | `Sink`     |
| TypeC actively sourcing                | `Sourcing`  | `Source`   |
| USB device (no PD)                     | `Connected` | `Unknown`  |

`power_role == DualRole` means "capable, not currently contracted."

### `link_speed_mbps: u32`, not `link_speed_bps: u64`

USB speeds are tier-discrete (1.5, 12, 480, 5000, 10000, 20000,
40000, 80000 Mbit/s). Sub-Mbps precision is wasted. NetworkManager
uses `u Mbit/s` for wired `Speed` — keep the GNOME stack consistent.
Zero when unknown.

### `usb_version: String` — canonical short form

Strip `"USB "` prefix from `LinkSpeed::label()` output. Examples:
`"2.0"`, `"3.0"`, `"3.1 Gen 2"`, `"3.2 Gen 2x2"`, `"4 Gen 3x2"`.
Empty string when unknown. The canonical form is **whatever
`LinkSpeed::label()` currently emits**, minus the prefix. Documented
in module docs.

### `vendor_id` / `product_id` as `q` (uint16), top-level

Raw 16-bit USB descriptor fields, top-level. UIs format
`${vid.toString(16).padStart(4,'0')}:${pid:04x}` on render.

`vid_pid` is **not** a property key anymore — fully hoisted.

### `vendor` / `product` top-level strings

Universal-ish, manufacturer-supplied prose, i18n-irrelevant. Hoisted
out of properties:

- `vendor: s` — manufacturer string from the USB descriptor, or
  vendor-DB lookup fallback. Empty when both fail.
- `product: s` — product string from the USB descriptor. Empty when
  unset.

Distinct from `headline` (daemon-composed display title) — frequently
similar but not identical. USBee renders `headline` as the row
title; `vendor` / `product` are for filters, search, and the detail
panel.

### `primary_driver: String` top-level

The kernel driver bound to the device's first interface. Empty when
no driver bound (a meaningful UI signal: "device attached, no
driver").

For multi-interface composite devices, additional drivers stay
discoverable via the `drivers` property (comma-joined list across all
interfaces).

This top-level field is what the `Serial` classifier reads.

### `device_class` — coarse enum, drops `TypeCPort`

`Keyboard` | `Mouse` | `InputTablet` | `Gamepad` | `Storage` |
`Display` | `Audio` | `Camera` | `Printer` | `Phone` | `Hub` |
`NetworkWired` | `NetworkWireless` | `SecurityKey` |
`SmartcardReader` | `Bluetooth` | `Serial` | `VideoCapture` |
`Unknown`

When `category == "TypeCPort"`, `device_class == "Unknown"`. No
two-dimensional state. (USBee's bug-class call.)

### `device_subclass: String` — advisory, daemon-curated

Open string field, non-binding. Day-one curated values per class:

| `device_class`     | day-one `device_subclass` values            |
|---|---|
| `Camera`           | `"webcam"` / `"capture"` / `""`             |
| `Storage`          | `"usb_stick"` / `"sd_reader"` / `""`        |
| `Audio`            | `"headset"` / `"microphone"` / `"dac"` / `""` |
| (others)           | `""` for now; additive later                |

UIs that don't care ignore the field. Empty string is the universal
fallthrough. Adding subclass values is non-breaking (§7).

**MUST NOT carry security-relevant information** (per USBee's
advisory clause).

### `power: (uus)` — `(power_in_mw, power_out_mw, power_role)`

- `power_in_mw: u` — power flowing into the host from this port (we
  are sinking). Sourced from the active PD sink contract — UCSI live
  `voltage × current` if available, else the negotiated PDO. **Zero
  for non-Type-C entries.** Plain USB device draw lives in the
  `usb_power_ma` property instead.
- `power_out_mw: u` — power flowing out of the host through this
  port (we are sourcing). Active PD source contract. Zero when not
  sourcing.
- `power_role: s` — per the table above.

Invariant: `power.power_in_mw > 0` ⟺ "this port is sinking PD power
right now." This is the signal USBee badges on.

### `usb_power_ma: u32` — descriptor draw for non-PD USB devices

Property key (not top-level). Raw `bMaxPower` value in milliamps.
Daemon does NOT compute watts. The descriptor field is advertised
against the bus's nominal 5 V (USB 2.0 / 3.x); UIs multiply by 5 for
W. Documented in CHANGELOG.

### Property keys — machine keys, daemon-owned vocabulary

Post-hoist (15 keys); top-level structured fields are not duplicated
in properties:

```
serial          mount             drivers
data_role       power_mode        pd_revision       plug_orientation
pd_contract     cable_speed       cable_current     cable_max_power
cable_type      cable_vendor      charger_max
usb_power_ma    (USB devices only)
```

Removed (now top-level): `vid_pid`, `vendor`, `product` (separately),
`primary_driver`.

Adding a property key is **additive** (non-breaking). Renaming or
removing is breaking — bump the interface.

USBee owns the key → display-label translation table.

### Method shape on Devices2

- `ListDevices() → a(...)` — full structured per-entry signature
- `Diagnose(port) → (bsssb)` — same `DiagnosticEntry` struct, no
  schema divergence from per-entry `charging_diag`
- `ListPorts() → ai` — unchanged
- `SnapshotJson() → s` — unchanged (full `serde_json` snapshot)
- `Refresh() → u` — unchanged
- `Version` / `DeviceCount` properties — unchanged
- `DeviceAdded` / `DeviceRemoved` / `CapabilityDegraded (iss)` /
  `CapabilityRestored (i)` signals — unchanged

### Wire signature

`ListDevices() → a(ssssssssss qq s a(ss) i u s (uus) (bsssb))`

19 top-level fields per entry, in order:

```
1.  id                    (s)
2.  category              (s)
3.  device_class          (s)
4.  device_subclass       (s)
5.  status                (s)
6.  headline              (s)
7.  subtitle              (s)
8.  icon                  (s)
9.  vendor                (s)
10. product               (s)
11. vendor_id             (q)   uint16
12. product_id            (q)   uint16
13. primary_driver        (s)
14. properties            a(ss)
15. port_number           (i)   int32
16. link_speed_mbps       (u)   uint32
17. usb_version           (s)
18. power                 (uus) (power_in_mw, power_out_mw, power_role)
19. charging_diag         (bsssb) (present, bottleneck, summary, detail, is_warning)
```

</decisions>

<classification>
## `device_class` classification policy (day-one fidelity)

| Class               | Signal                                                          | Fidelity |
|---|---|---|
| `Keyboard`          | Interface class `0x03` (HID), protocol `0x01`                   | high   |
| `Mouse`             | Interface class `0x03` (HID), protocol `0x02`                   | high   |
| `Gamepad`           | Interface class `0x03` (HID), product string heuristic           | medium |
| `InputTablet`       | Interface class `0x03` (HID), product string heuristic           | medium |
| `Storage`           | Interface class `0x08` (Mass Storage)                            | high   |
| `Display`           | Interface class `0x0E` Video subclass `0x02` (Video Streaming) on a non-camera-typed device + product heuristics (`monitor`, `display`) | low — fall through to `Unknown` if uncertain |
| `Audio`             | Interface class `0x01` (Audio)                                   | high   |
| `Camera`            | Interface class `0x0E` (Video)                                   | medium — `VideoCapture` siblings |
| `VideoCapture`      | Interface class `0x0E` + product string match `capture\|hdmi\|elgato\|cam link\|magewell\|av\.io\|epiphan` | medium — heuristic |
| `Printer`           | Interface class `0x07` (Printer)                                 | high   |
| `Phone`             | Vendor heuristic (Apple iPhone) + product string `android`       | high   |
| `Hub`               | `device_class == 0x09`                                           | high   |
| `NetworkWired`      | Interface class `0x02` (CDC) subclass `0x06` (Ethernet) OR class `0xFF` + driver in `{cdc_ether, r8152, asix, ax88179_178a}` | medium |
| `NetworkWireless`   | Interface class `0xE0` (Wireless), subclass `!= 0x01` (excluding BT) | medium |
| `SecurityKey`       | Existing icon-resolution heuristic (Yubico VID `0x1050`, Nitrokey VID `0x20A0`, product match `yubikey\|security key\|nitrokey\|solokey\|onlykey\|titan security\|fido`) | high |
| `SmartcardReader`   | Interface class `0x0B` (Smart Card / CCID)                       | high   |
| `Bluetooth`         | Interface class `0xE0` (Wireless), subclass `0x01` (Radio Frequency), protocol `0x01` (Bluetooth Programming Interface) | high |
| `Serial`            | Interface class `0x02` (CDC) subclass `0x02` (ACM) OR `primary_driver` ∈ `{ftdi_sio, cp210x, ch341, pl2303, ti_usb_3410, mos7720, mos7840}` | high |
| `Unknown`           | fallthrough                                                      | n/a    |

USBee's "Unknown rate at launch is a useful proxy for whether the
enum is granular enough" is the watch metric. Day-one false-Unknowns
expected for: capture cards we don't recognize, vendor-specific
serial chips, exotic input devices.

</classification>

<specifics>
## Specific references in scope

USBee regex set → Devices2 field map (this table goes verbatim into
the CHANGELOG migration entry):

| Today's regex / heuristic                       | Replaced by                          |
|---|---|
| `WATT_RE` on bullets                            | `power.power_in_mw / power_out_mw`   |
| `DIRECTION_RE` (`sink`/`source`)                | `power.power_role`                   |
| `USB_VERSION_RE` (+ "USB 3xtra" patch)          | `usb_version`                        |
| `SPEED_RE` (`Gb`/`Mb`/`Kb`/s)                   | `link_speed_mbps`                    |
| substring `limited to`/`swap`/`degraded`        | `charging_diag.is_warning`           |
| per-bullet label-classifier regex               | `properties[i].0` (machine key)      |
| headline substring → icon                       | `device_class` enum + `icon` string  |

</specifics>

<canonical_refs>
## Canonical references

- `.planning/quick/20260513-devices2-structured-wire/CONTEXT.md`
  (this file — authoritative spec)
- `src/summary.rs` — `DeviceSummary` refactor
- `src/dbus.rs` — D-Bus interface module to rewrite
- `src/output.rs` — owner of English display-label map after refactor
- `src/usb.rs` — `UsbDevice` source data; `DeviceClass` enum lands
  here (or new module)
- `src/diagnostic.rs` — `ChargingDiagnostic` (already structured;
  wire serialization stays at `(bsssb)`)
- `src/typec.rs`, `src/power.rs`, `src/cable.rs` — source data for
  the structured fields
- `CHANGELOG.md` — `[Unreleased]` gets the v2 migration note with
  full regex→field table
- `README.md` — wire-surface table + USBee description
- `examples/dbus_client.rs` — must demonstrate every new field
- `tests/dbus_interface.rs` — fixture-driven integration tests

`zbus` generates introspection XML at runtime from the `#[interface]`
macro. The acceptance criterion "dbus-iface.xml carries Devices2"
maps to: `gdbus introspect --session --dest org.usbeehive.Devices
--object-path /org/usbeehive/Devices` shows the Devices2 schema.

</canonical_refs>

<deferred>
## Deferred (out of scope this phase)

- Per-property markup (warnings / highlights / colors on individual
  values). Plain strings only.
- Non-USB-C diagnostics ("USB-A device drawing more than the hub
  advertises").
- Speculative diagnostics ("would charge faster on a PD port").
- i18n of `headline` / `subtitle` — daemon prose stays English.
  Structured enum values stay English variant names.
- Versioned introspection annotations (`<version>` in XML). `Version`
  property already covers capability gating.
- `CapabilityDegraded` signal payload restructure. Stays `(iss)`
  even though it duplicates fields available on `entry.charging_diag`
  — the signal is the notification trigger and needs to be
  self-contained.

</deferred>

<acceptance>
## Acceptance (what "done" means)

1. `org.usbeehive.Devices2` is the only interface registered;
   `Devices1` removed.
2. `ListDevices()` returns the locked 19-field tuple shape; every
   USB device, hub, and Type-C port populates the structured fields
   per the policy above.
3. `Diagnose(port) → (bsssb)` (present bool + 4 strings/bool).
4. `DeviceSummary` lib type carries `properties: Vec<(String, String)>`
   instead of `bullets: Vec<String>`, plus all new structured fields.
5. `usbeehive` CLI text output renders properties via an English
   key→label map in `src/output.rs`. No behavioral regression visible
   to existing CLI users.
6. `usbeehive --json` output replaces `bullets` with `properties`,
   exposes new top-level fields. Documented in CHANGELOG.
7. New `Status::Sourcing` variant fires when host sources to a
   downstream device.
8. `cargo build --all-features` clean.
9. `cargo test --all-features` clean. New fixture-driven tests cover:
   - USB-device-only (no Type-C, all USB fields populated)
   - Empty Type-C port
   - Cable-limit charging (existing fixture, asserts new shape)
   - Sourcing-to-partner
   - No-driver-bound (`primary_driver == ""`)
   - Smartcard CCID (`device_class == "SmartcardReader"`)
   - Serial CDC ACM (`device_class == "Serial"`)
10. `cargo test --no-default-features` clean.
11. `cargo clippy --all-targets --all-features -- -D warnings` clean.
12. `cargo fmt --all -- --check` clean.
13. `CHANGELOG.md [Unreleased]` includes:
    - "Changed (breaking — D-Bus + CLI JSON)" entry
    - Full regex → field mapping table
    - Bullet-label → property-key mapping table
    - Enum-extensibility convention call-out
    - USBee migration pointer
14. `README.md` D-Bus wire-surface table updated to Devices2 shape.
15. `examples/dbus_client.rs` prints every new top-level field from
    `ListDevices` plus `Diagnose(0)`.

Live `busctl --user call` against the daemon is **not** part of CI
but should be smoke-tested post-merge.

</acceptance>

## Next step

PLAN.md decomposes the refactor into atomic stages. Execute inline —
no executor agent spawn (the task fits in one focused session).
