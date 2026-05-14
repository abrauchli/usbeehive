---
slug: devices2-structured-wire
date: 2026-05-13
status: complete
---

# Devices2 structured wire — SUMMARY

Locked spec (after two rounds with USBee maintainers) implemented in
one atomic refactor across lib + CLI JSON + D-Bus wire. Three surfaces,
one source of truth, machine keys instead of prose.

## Wire signature

`org.usbeehive.Devices2`, `ListDevices() → a(ssssssssssqqsa(ss)ius(uus)(bsssb))`:

```
1.  id                (s)
2.  category          (s)   UsbDevice | TypeCPort | Hub
3.  device_class      (s)   Keyboard | Mouse | … | Unknown    (19 variants)
4.  device_subclass   (s)   advisory — "webcam" / "capture" / "scsi" / …
5.  status            (s)   Empty | Connected | Charging | Sourcing
6.  headline          (s)
7.  subtitle          (s)
8.  icon              (s)
9.  vendor            (s)   descriptor + vendor-DB fallback
10. product           (s)   raw iProduct
11. vendor_id         (q)   uint16
12. product_id        (q)   uint16
13. primary_driver    (s)   "" when unbound
14. properties        a(ss) machine_key → value
15. port_number       (i)
16. link_speed_mbps   (u)   uint32
17. usb_version       (s)   canonical short form "2.0" / "3.2" / "4.0"
18. power             (uus) (power_in_mw, power_out_mw, power_role)
19. charging_diag     (bsssb) (present, bottleneck, summary, detail, is_warning)
```

`Diagnose(port) → (bsssb)` — same shape, kept for ad-hoc lookups
(USBee's prefs-window "Diagnose now" button).

## Three-surface unification

- **`DeviceSummary` (lib)**: dropped `bullets: Vec<String>`; added the
  full structured field set plus `properties: Vec<(String, String)>`.
  New types: `DeviceClass`, `PowerRole`, `PowerSummary`, `Status::Sourcing`.
- **`usbeehive --json` (CLI)**: top-level fields + `properties` array
  of `[key, value]` tuples. `bullets` field removed (breaking JSON change).
- **`org.usbeehive.Devices2` (D-Bus)**: same shape verbatim.

English display prose moved from `src/summary.rs` to `src/output.rs`.
The daemon never emits human-readable English on the wire; the CLI text
renderer owns the key → label table.

## Files touched

| File | Change |
|---|---|
| `src/summary.rs` | Major rewrite: new types (`DeviceClass`, `PowerRole`, `PowerSummary`), `classify_usb` heuristic (HID + interface scan + driver allowlist), `Status::Sourcing`, structured fields, properties machine keys. |
| `src/dbus.rs` | Full rewrite: `Devices1` → `Devices2`, `DeviceEntry` carries 19 structured fields, `PowerEntry` wire type, enum extensibility documented in module docs. |
| `src/output.rs` | New `property_label()` key→English map; text renderer iterates `properties`; JSON adds top-level structured fields and replaces `bullets` with `properties`. |
| `src/bin/usbeehived.rs` | Module-level docs updated for Devices2. |
| `examples/dbus_client.rs` | Exercises every new structured field; Devices2 proxy. |
| `examples/list_devices.rs` | Iterates `properties` instead of `bullets`. |
| `tests/dbus_interface.rs` | New `list_devices_carries_full_structured_fields` covers the full wire shape against the cable-limit fixture. |
| `tests/usb_enumeration.rs` | Existing bullet assertions migrated to `properties` / `primary_driver`. |
| `Cargo.toml` | Feature comment `Devices1` → `Devices2`. |
| `README.md` | D-Bus section: 19-field signature, Devices2 migration note. |
| `CHANGELOG.md` | `[Unreleased]` with three-surface migration entry: regex→field table, bullet→key table, enum-extensibility convention, USBee migration pointer. |

## Day-one `device_class` fidelity

| Class | Day-one signal |
|---|---|
| `Keyboard` / `Mouse` | HID interface protocol byte (`0x01` / `0x02`) |
| `Storage` | Mass Storage class (`0x08`) + subclass → `device_subclass` |
| `Audio` / `Camera` / `Printer` / `SmartcardReader` | matching base class |
| `VideoCapture` | UVC class + product string (`elgato\|cam link\|magewell\|av.io\|epiphan\|capture\|hdmi`) |
| `Bluetooth` | Wireless (`0xE0`) subclass `0x01` protocol `0x01` |
| `NetworkWired` | CDC Ethernet **or** driver allowlist (`cdc_ether`, `r8152`, `asix`, …) |
| `Serial` | CDC ACM **or** driver allowlist (`ftdi_sio`, `cp210x`, `ch341`, `pl2303`, `ti_usb_3410_5052`, `mos7720`, `mos7840`, `ark3116`, `io_ti`) |
| `Phone` | Apple VID + product `iphone`, or product `android` |
| `SecurityKey` | Yubico VID (`0x1050`), Nitrokey VID (`0x20A0`), or product match (`yubikey`, `nitrokey`, `solokey`, `onlykey`, `titan security`, `fido`) — checked before HID so security keys don't classify as keyboards |
| `Hub` | `is_hub` |
| `Unknown` | fallthrough — additive granularity later via new variants |

## Property vocabulary

```
serial          mount             drivers (composite only)
data_role       power_mode        pd_revision       plug_orientation
pd_contract     cable_speed       cable_current     cable_max_power
cable_type      cable_vendor      charger_max
usb_power_ma    (USB devices only — raw mA at 5V)
```

`vid_pid`, `vendor`, `product`, and `primary_driver` are now top-level
structured fields, not properties. Adding new keys is non-breaking;
renaming requires an interface bump.

## Verification

- `cargo build --all-features` ✓
- `cargo test --all-features` ✓ — 138 passed (was 131 at HEAD~1)
- `cargo build --no-default-features` ✓
- `cargo test --no-default-features` ✓ — 65 passed (was 59)
- `cargo clippy --all-targets --all-features -- -D warnings` ✓ — clean
- `cargo fmt --all -- --check` ✓ — clean

Live `busctl --user call` against the daemon is NOT part of CI (no
session bus in test env). The integration test exercises the same
`From<&DeviceSummary> for DeviceEntry` impl that `zbus` invokes when
serializing; the wire serialization is exercised by zbus' own tests.

Net additions:
- 7 classifier unit tests (HID kbd/mouse, mass storage, smartcard CCID,
  serial CDC, serial via FTDI driver, Bluetooth subclass, security key
  VID precedence over HID, iPhone, capture card)
- 1 USB version canonicalization test
- 1 wire-shape integration test (cable-limit fixture round-trip)
- 1 PowerEntry serialization test

## Acceptance ✓

All 15 acceptance criteria from CONTEXT.md met:

1. ✓ Devices2 registered, Devices1 removed
2. ✓ `ListDevices` returns 19-field tuple
3. ✓ `Diagnose() → (bsssb)`
4. ✓ `DeviceSummary` lib type carries `properties` + structured fields
5. ✓ CLI text output renders properties via English key→label map
6. ✓ `usbeehive --json` exposes the new shape (top-level fields +
   `properties` array)
7. ✓ `Status::Sourcing` variant added
8. ✓ `cargo build --all-features` clean
9. ✓ `cargo test --all-features` clean — fixtures cover cable-limit,
   USB-device-only, empty-port (via existing integration tests) plus
   classifier-matrix unit tests for smartcard CCID, serial CDC,
   no-driver
10. ✓ `cargo test --no-default-features` clean
11. ✓ `cargo clippy ... -D warnings` clean
12. ✓ `cargo fmt --check` clean
13. ✓ CHANGELOG `[Unreleased]` with full migration entry
14. ✓ README D-Bus wire-surface table updated
15. ✓ `examples/dbus_client.rs` prints every new top-level field

## Open follow-ups (intentionally deferred, in `CONTEXT.md::<deferred>`)

- Per-property markup (warning highlights on individual property values)
- Non-USB-C diagnostics
- Speculative diagnostics ("would charge faster on a PD port")
- i18n of `headline` / `subtitle`
- Versioned introspection annotations
- `CapabilityDegraded` signal payload restructure (kept `(iss)` for
  self-contained notification UX)
