---
status: complete
date: 2026-06-10
task: 260610-port-usb-correlation
repos:
  usbeehive: /home/blk/projects/rust/usbeehive
  usbee: /home/blk/projects/rust/usbee
commits:
  task1_usbeehive: fd87c12
  task2_usbeehive: a323834
  task3_usbeehive: 89af1e0
  task4_usbee: 38c29f6
---

# Summary: Type-C port <-> enumerated USB device correlation

## What was built

Four atomic tasks shipped across two repos (usbeehive + usbee).

### Task 1 - Parse partner USB child node + correlate in build_summaries

Commit: fd87c12 (usbeehive)

- TypeCPartner gains usb_name: String (#[serde(default)], empty when no USB
  child node) - the basename of the kernel's bus-port child directory under
  the partner sysfs dir (e.g. "2-2"), the canonical port<->USB-device linkage.
- partner_usb_name() helper scans partner subdirs for the first (sorted) entry
  matching <digits>-<port>(\.<digits>)* using byte checks (no regex).
- is_bus_port_name() predicate validates the pattern.
- build_summaries computes usb_match via partner.usb_name and passes it to
  from_typec_port. Charger partners have empty usb_name - lookup is a silent no-op.
- link_partner_usb fixture helper creates the USB child dir as a plain subdir.
- Tests: is_bus_port_name variants, partner_usb_name with real tempdir fixtures,
  manager correlation + charger no-op regression.

Files: src/typec.rs, src/sysfs/typec.rs, src/sysfs/manager.rs, tests/fixture_builder.rs

### Task 2 - USB-string subtitle fallback + usb_device cross-ref property

Commit: a323834 (usbeehive)

- from_typec_port gains a fourth usb: Option<&UsbDevice> parameter.
- Subtitle precedence: (1) identity VDO label, (2) USB product/manufacturer
  string (e.g. "Pixel 7"), (3) "Device connected" fallback.
- New usb_device property emits usb:<bus_port> when a USB device is correlated.
- src/dbus.rs: documents additive usb_device key and usb_name JSON field.
- src/output.rs: property_label maps "usb_device" -> "USB device".
- Tests: subtitle=product when no identity, identity wins with USB device,
  empty strings fall to "Device connected", manufacturer+product combined.

Files: src/summary.rs, src/dbus.rs, src/output.rs

### Task 3 - Strengthen cable.data_speed_limit with negotiated link speed

Commit: 89af1e0 (usbeehive)

- negotiated_cable_speed(mbps) -> Option<CableSpeed> helper maps USB link
  speed in Mbps to CableSpeed tiers. Returns None for 0.
- Trigger (b) (negotiated <= cable_speed) is a strict subset of trigger (a)
  (VDO-based); emit when VDO trigger fires. Negotiated-only is NOT a trigger
  (Pixel-7 no-identity guard).
- Tests: threshold mapping, Gen2+USB2+480Mbps fires, Gen2+Gen2+10000 absent,
  no-VDO+480 absent, inconsistent-data still fires via VDO path.
- CHANGELOG updated under existing [0.10.0] Added section.

Files: src/summary.rs, CHANGELOG.md

### Task 4 - usbee label-table + CHANGELOG (sister repo)

Commit: 38c29f6 (usbee)

- label-table.js: added ['usb_device', _('USB device')] in additive-keys group.
- po/usbee@bitcreed.us.pot: hand-added msgid "USB device" entry.
- CHANGELOG.md [2.4.0] Added section: documents correlation feature, subtitle
  change, usb_device property; confirms no wire change, MIN_USBEEHIVE_VERSION
  stays 0.10.0.

Files: usbee@bitcreed.us/src/label-table.js, po/usbee@bitcreed.us.pot, CHANGELOG.md

## Live verification

cargo run --bin usbeehive -- --list was run after Task 2 and after Task 3.
Both times, neither USB-C port had an attached partner. The Pixel 7 and charger
were not connected at execution time.

Output observed:
- Port 0: "Nothing connected" (no partner dir present in sysfs)
- Port 1: "Nothing connected" (no partner dir present in sysfs)
- Internal USB devices (fingerprint sensor, camera) rendered unchanged.

The subtitle correlation is verified by unit tests (171 pass). The live output
is consistent with the no-op path for ports with no partner.

## Verification gates

- cargo fmt: clean
- cargo test --all-features: 171 passed, 0 failed
- cargo clippy --all-features -- -D warnings: clean
- node -c label-table.js: SYNTAX OK

## Deviations

None. Plan executed exactly as written.
