# D-Bus consumer spec — BOS capability vs negotiated data rate

**Audience:** the author of a client that consumes `org.usbeehive.Devices5`
(e.g. the `usbee` GNOME Shell extension) and cannot see the daemon's source.

**Status:** implemented, unreleased. Present from the next `usbeehive`
release after 0.11.0.

**Interface version: `org.usbeehive.Devices5` — UNCHANGED.**
This is a purely additive change. There is no `Devices6`, no bus-name change,
no object-path change, and no change to any existing method's, property's, or
signal's signature. A client that ignores everything in this document keeps
working byte-for-byte as before.

---

## 1. What this adds, in one paragraph

`usbeehive` previously reported only what a USB link **negotiated**
(`link_speed_mbps`, from sysfs `speed`). It now also decodes
`/sys/bus/usb/devices/<dev>/bos_descriptors` — the device's **Binary Object
Store** — which says what the device is **capable of**, independent of the
port it is plugged into. The difference between the two is the "you plugged
your 5 Gbps device into a 480 Mbps port" verdict. It is surfaced as new
`properties` keys, a typed `data_rate` object in `SnapshotJson`, and two new
signals.

---

## 2. Wire-surface delta

### 2.1 Nothing changed

| Member | Signature | Change |
|---|---|---|
| `ListDevices` | `() → a(ssssssssssqqsa(ss)ius(uuus)(bsssb)a(usuuuub)i)` | **none** |
| `ListPorts` | `() → ai` | none |
| `Diagnose` | `(i) → (bsssb)` | none |
| `SnapshotJson` | `() → s` | JSON gains fields (§4); D-Bus signature unchanged |
| `Refresh` | `() → u` | none |
| `Version`, `DeviceCount` | `s`, `u` | none |
| `DeviceAdded` `(ss)`, `DeviceRemoved` `(s)`, `DeviceChanged` `(s)` | | none |
| `CapabilityDegraded` `(iss)`, `CapabilityRestored` `(i)` | | none |

The per-entry tuple shape is **not** touched. In particular the `power` tuple
stays `(uuus)`. Positional unpackers need no change.

### 2.2 New signals

| Signal | Signature | Args |
|---|---|---|
| `DataRateDegraded` | `(sss)` | `(id, summary, detail)` |
| `DataRateRestored` | `(s)` | `(id)` |

Both are keyed on the **summary `id` string** — the same `id` that is field 1
of a `ListDevices` entry (`usb:<bus_port>`, e.g. `usb:5-2.1.1`) — **not** on a
Type-C port number.

> **Why not reuse `CapabilityDegraded (iss)`?** That signal's first argument
> is an `i` Type-C **port number**, and its payload comes from the *charging*
> diagnostic. A data-rate shortfall almost always lands on a plain USB device
> several hops behind a hub, which has no Type-C port number at all — there is
> literally no `i` to send. The two are separate concepts (power vs data) with
> separate keys, and clients should render them separately.

**Emission semantics** (identical in shape to `CapabilityDegraded`):

- `DataRateDegraded` fires on the enumeration pass where a device's data-rate
  warning flips from off to on, **or** where a device appears for the first
  time already warning. It does **not** repeat on subsequent passes while the
  condition persists.
- `DataRateRestored` fires on the pass where a previously-warning device stops
  warning. A device that disappears entirely emits `DeviceRemoved`, **not**
  `DataRateRestored`.
- `summary` and `detail` duplicate `data_rate.summary` / `data_rate.detail`
  (§4) so the signal is self-contained for a notification. Both are English
  prose intended for display; `detail` may be the empty string.
- Enumeration is driven by libudev hot-plug plus a poll; a device that is
  moved from a slow port to a fast one produces `DeviceRemoved` +
  `DeviceAdded` (different `id`), not a `DataRateRestored` on the old id.
  `DataRateRestored` is for the in-place case (e.g. a link renegotiating).
- A verdict change also flips the curated state fingerprint, so
  **`DeviceChanged` fires too** for the same `id`. A client that already
  re-snapshots on `DeviceChanged` needs no new subscription to stay correct —
  the new signals exist to make a *notification* possible, not to make
  correctness possible.

Unknown signals are silently ignored by D-Bus clients, so subscribing is
optional.

---

## 3. New `properties` keys

`properties` is field 14 of a `ListDevices` entry, of type `a(ss)` —
`(machine_key, value)` pairs. Adding keys is explicitly documented as
non-breaking on this interface. All values are **UTF-8 strings**, including
the numeric ones.

These keys appear only on entries whose `category` is `UsbDevice` or `Hub`.
They **never** appear on `TypeCPort` entries — the BOS belongs to the
enumerated USB device, which has its own entry (correlated from the port via
the existing `usb_device` property).

**Every key below is optional.** Absence is normal and load-bearing; see §5.

| Machine key | Value format | Units | Proposed English display label |
|---|---|---|---|
| `usb_capable_speed_mbps` | decimal integer, e.g. `"5000"` | Mbps | Device capable of (Mbps) |
| `usb_capable_speed` | label string, e.g. `"SuperSpeed 5 Gbps"` | — | Device capable of |
| `usb_capable_gen` | `"Gen 1x1"` \| `"Gen 1x2"` \| `"Gen 2x1"` \| `"Gen 2x2"` \| `"Gen 3x1"` \| `"Gen 3x2"` | — | Device generation |
| `usb_capable_rx_lanes` | decimal integer, e.g. `"1"` | lanes | Capable RX lanes |
| `usb_capable_tx_lanes` | decimal integer, e.g. `"1"` | lanes | Capable TX lanes |
| `usb_functional_floor_mbps` | decimal integer, e.g. `"480"` | Mbps | Needs at least (Mbps) |
| `usb_link_verdict` | `"AtCapability"` \| `"BelowCapability"` \| `"Degraded"` | — | Link vs capability |
| `usb_link_degraded` | **flag key** — always the literal `"true"` | — | Linked below this device's own requirement |
| `usb_bos_suppressed` | **flag key** — always the literal `"true"` | — | Capability unknown (kernel suppresses BOS for this device) |
| `usb_bos_container_id` | lowercase hyphenated UUID, e.g. `"f1adf5ec-1150-0540-91ec-71ca7101b6a2"` | — | Container ID |
| `usb_altmode_svids` | comma-separated 4-digit lowercase hex, e.g. `"ff01"` or `"ff01,8087"` | — | Alt-mode SVIDs |
| `usb_altmode_state` | `"Successful"` \| `"Unsuccessful"` \| `"NotAttempted"` \| `"UnspecifiedError"` | — | Alt-mode state |
| `usb_altmode_failure` | comma-separated reasons: `"no_usb_pd"`, `"no_battery"` | — | Alt-mode failure |

### 3.1 Flag keys

`usb_link_degraded` and `usb_bos_suppressed` follow the interface's existing
flag-key convention (as used by the `transport.*` and `cable.trust.*`
families): **the key is present only when the condition is true.** Its value
is always the literal string `"true"`. Absence means false. Do not render
them as `key: true`; render the label alone, or nothing.

`usb_link_degraded` is present **exactly when** `usb_link_verdict ==
"Degraded"`. It is redundant with the verdict, and exists so a client can
key a warning badge off a single presence check.

### 3.2 Enum-value extensibility

`usb_link_verdict` and `usb_altmode_state` follow the interface's existing
enum-extensibility convention: **new values may be added without an interface
bump.** Clients MUST treat an unrecognised value as "unknown" and fall back to
neutral rendering. Removing or renaming a value would require an interface
bump.

`usb_altmode_failure` is a comma-separated *list*; new reason tokens may be
added. Parse by splitting on `,` and ignoring unknown tokens.

### 3.3 SVID values worth naming

`usb_altmode_svids` carries raw SVIDs. Two are worth a display name:

| SVID | Name |
|---|---|
| `ff01` | DisplayPort |
| `8087` | Thunderbolt |

Anything else should be rendered as the bare hex.

---

## 4. `SnapshotJson` additions

`SnapshotJson()` returns `serde_json` of the array of internal
`DeviceSummary` values. Three additions, all additive with serde defaults, in
the same style as the `TypeCPartner.usb_name` addition in 0.10.0:

### 4.1 `data_rate` — per summary entry

Sibling of the existing `charging_diag`. `null` when the device published no
BOS, and on every Type-C port entry.

```json
"data_rate": {
  "verdict": "BelowCapability",
  "negotiated_mbps": 480,
  "capable_mbps": 5000,
  "functional_floor_mbps": 480,
  "capable_gen": "",
  "summary": "Linked at 480 Mbps of an advertised 5 Gbps",
  "detail": "Vendor declares full functionality from 480 Mbps — not a fault",
  "is_warning": false
}
```

| Field | Type | Meaning |
|---|---|---|
| `verdict` | string | `AtCapability` \| `BelowCapability` \| `Degraded` \| `Unknown` |
| `negotiated_mbps` | integer | Mbps the link actually negotiated (== `link_speed_mbps`) |
| `capable_mbps` | integer | Highest aggregate rate advertised. `0` = the BOS carries no speed-bearing capability |
| `functional_floor_mbps` | integer | Vendor's declared floor. `0` = undeclared |
| `capable_gen` | string | `"Gen 2x1"` etc.; `""` when not derivable |
| `summary` | string | English prose; `""` when `verdict == "Unknown"` |
| `detail` | string | English prose; may be `""` |
| `is_warning` | bool | `true` **only** for `verdict == "Degraded"` |

Note that `verdict: "Unknown"` is exposed in `SnapshotJson` but **not** in the
`properties` bag — `usb_link_verdict` is simply absent in that case.

### 4.2 `usb_device.bos` — the raw decoded BOS

`null` when absent. Structure:

```json
"bos": {
  "num_device_caps": 2,
  "total_length": 22,
  "truncated": false,
  "capabilities": [
    { "kind": "Usb2Extension", "bm_attributes": 6, "lpm": true, "besl": true,
      "baseline_besl_valid": false, "deep_besl_valid": false,
      "baseline_besl": 0, "deep_besl": 0 },
    { "kind": "SuperSpeed", "ltm_capable": true, "speeds_supported": 14,
      "functionality_support": 2, "u1_dev_exit_lat_us": 10,
      "u2_dev_exit_lat_us": 2047 }
  ]
}
```

`capabilities` is an internally-tagged array; the `kind` discriminator is one
of `Usb2Extension`, `SuperSpeed`, `ContainerId`, `SuperSpeedPlus`,
`Billboard`, `Other`. **New `kind` values may be added** — treat unknown
kinds as opaque. `Other` carries `{ "cap_type": <int>, "data": [<bytes>] }`
for capability types this daemon does not decode, so nothing from the wire is
lost.

`truncated: true` means the blob was cut short or malformed; `capabilities`
then holds whatever decoded cleanly before the fault. Clients should treat a
truncated BOS as lower-confidence but not as an error.

Most clients should read `data_rate` and the `properties` keys and ignore
`bos` entirely. It is there for diagnostics and bug reports.

### 4.3 `usb_device.quirks`

Integer, `0` when the kernel reports no quirks. This is the raw
`/sys/bus/usb/devices/<dev>/quirks` bitmask. Only one bit matters to a client:

| Bit | Constant | Meaning |
|---|---|---|
| 17 (`0x20000`) | `USB_QUIRK_NO_BOS` | The kernel deliberately never reads this device's BOS |

See §5.2.

---

## 5. What is guaranteed absent, and when

This section is the important one. **A missing key is information, not a
failure.**

### 5.1 USB 2.0-only devices — no BOS at all

The BOS was introduced with USB 3.0. A USB 2.0-only device publishes none and
the kernel does not create the `bos_descriptors` sysfs file. For such a
device:

- `data_rate` is `null` in `SnapshotJson`;
- `usb_device.bos` is `null`;
- **every** key in §3 is absent from `properties`;
- no `DataRateDegraded` signal will ever fire for it.

**Do not render this as a problem.** It is the single most common case. On a
typical desktop the majority of USB entries look exactly like this.

### 5.2 Quirked devices — no BOS *despite* being capable

If `usb_device.quirks & 0x20000` (`USB_QUIRK_NO_BOS`), the kernel refuses to
read the device's BOS because the device is known to mishandle the request.
The BOS is then absent **regardless of what the device can actually do** — a
SuperSpeed-capable device can land here.

The daemon surfaces this as the flag key `usb_bos_suppressed = "true"`,
present only in this case. When you see it:

- capability is **UNKNOWN**, not "USB 2.0 only";
- never infer a shortfall, never draw a warning, never say "this device is
  slow";
- if you show anything at all, say the capability could not be determined.

The daemon itself produces no verdict for such a device (`data_rate` is
`null`), so it can never be reported as degraded — but a client that
independently reasoned "no BOS ⟹ USB 2.0" would get it wrong, which is why
the flag exists.

### 5.3 A BOS with no speed-bearing capability

A device may publish a BOS containing only, say, a USB 2.0 Extension or a
Billboard capability. Then:

- `data_rate` is **present** with `verdict: "Unknown"`, `capable_mbps: 0`,
  and empty `summary` / `detail`;
- `usb_capable_speed_mbps`, `usb_capable_speed` and `usb_link_verdict` are
  **absent** from `properties`;
- `usb_link_degraded` is absent.

Render nothing.

### 5.4 Capability fields that individual devices omit

- `usb_capable_gen`, `usb_capable_rx_lanes`, `usb_capable_tx_lanes` come from
  the **SuperSpeedPlus** capability (`0x0A`). A plain SuperSpeed (5 Gbps)
  device has no such capability, so these three are absent even though
  `usb_capable_speed_mbps` is present. This is normal.
- `usb_functional_floor_mbps` is absent when the device declares no
  `bFunctionalitySupport`, or when it declares one that contradicts its own
  `wSpeedsSupported` bitmap (the daemon refuses to trust that).
- `usb_bos_container_id` is absent when the Container ID capability is missing
  **or is all zeros** — some devices publish a zero UUID as a placeholder,
  which identifies nothing.
- `usb_altmode_*` are absent unless the device published a Billboard
  capability (`0x0D`). Only USB-C docks / monitors / dongles do.
- `usb_altmode_failure` is absent when `bAdditionalFailureInfo` is zero.

### 5.5 Type-C port entries

Entries with `category == "TypeCPort"` never carry any key from §3 and always
have `data_rate: null`. If you want a port's data-rate story, follow the
existing `usb_device` property on the port entry to the corresponding
`usb:<bus_port>` entry.

---

## 6. The false-positive rule — read this before drawing a warning

`usb_link_degraded` / `verdict == "Degraded"` is **deliberately conservative**
and the client should not try to be cleverer than it.

A device's BOS carries two separate numbers:

- `wSpeedsSupported` → "what I *can* do" → `usb_capable_speed_mbps`
- `bFunctionalitySupport` → "the slowest speed at which I am *fully
  functional*, according to my own vendor" → `usb_functional_floor_mbps`

The daemon flags **only** `negotiated < functional_floor`. A device that
advertises SuperSpeed but declares full functionality at High Speed, and is
linked at High Speed, is reported as `BelowCapability` — **informational, not
a warning** — because its own maker says it is fine there. Flagging it would
produce a warning on a large fraction of ordinary hardware.

Recommended client rendering:

| verdict | Suggested treatment |
|---|---|
| `AtCapability` | Neutral, or nothing. Optionally a quiet ✓. |
| `BelowCapability` | Informational only. Neutral colour, no badge, no notification. Good place for "could go faster elsewhere" copy if you want it, phrased as a possibility, not a fault. |
| `Degraded` | Warning. This is the actionable "wrong port / wrong cable" case. Badge + notification are appropriate. |
| `Unknown` / absent | Render nothing. |

Do **not** synthesise your own warning from
`usb_capable_speed_mbps > link_speed_mbps`. That comparison is exactly the
noisy false positive this design avoids: on the reference machine below it
would fire on 2 of 2 devices, both of which are working correctly.

---

## 7. Worked examples — real devices

All values below were captured live from the development machine.

### 7.1 `usb:5-2.1.1` — "USB 10/100/1000 LAN", SuperSpeed device on a USB 2.0 hub

Raw blob: `05 0f 16 00 02 | 07 10 02 06 00 00 00 | 0a 10 03 02 0e 00 02 0a ff 07`

`ListDevices` entry (relevant fields):

```
id              = "usb:5-2.1.1"
category        = "UsbDevice"
headline        = "USB 10/100/1000 LAN"
link_speed_mbps = 480
usb_version     = "2.1"
properties      = [
    ("usb_max_power_ma",          "350"),
    ("transport.usb2",            "true"),
    ("usb_capable_speed_mbps",    "5000"),
    ("usb_capable_speed",         "SuperSpeed 5 Gbps"),
    ("usb_functional_floor_mbps", "480"),
    ("usb_link_verdict",          "BelowCapability"),
]
```

Note what is **absent**: `usb_link_degraded`, `usb_capable_gen`,
`usb_capable_rx_lanes`, `usb_capable_tx_lanes`, `usb_bos_container_id`.

`SnapshotJson` `data_rate`:

```json
{ "verdict": "BelowCapability", "negotiated_mbps": 480, "capable_mbps": 5000,
  "functional_floor_mbps": 480, "capable_gen": "",
  "summary": "Linked at 480 Mbps of an advertised 5 Gbps",
  "detail": "Vendor declares full functionality from 480 Mbps — not a fault",
  "is_warning": false }
```

This device is physically on the wrong port — it could run at 5 Gbps
elsewhere. It is still **not** flagged, because its own descriptor says High
Speed is enough for full function. No `DataRateDegraded` signal is emitted.

Its `usb_device.quirks` is `1024` (`USB_QUIRK_NO_LPM`) — an unrelated quirk;
`0x20000` is clear, so `usb_bos_suppressed` is absent.

### 7.2 `usb:5-2` — "4-Port USB 2.0 Hub"

Raw blob: `05 0f 2a 00 03 | 07 10 02 1e f4 00 00 | 0a 10 03 00 0e 00 01 0a ff 03 | 14 10 04 00 <16-byte uuid>`

```
id              = "usb:5-2"
category        = "Hub"
link_speed_mbps = 480
properties      = [
    …,
    ("usb_bos_container_id",      "f1adf5ec-1150-0540-91ec-71ca7101b6a2"),
    ("usb_capable_speed_mbps",    "5000"),
    ("usb_capable_speed",         "SuperSpeed 5 Gbps"),
    ("usb_functional_floor_mbps", "12"),
    ("usb_link_verdict",          "BelowCapability"),
]
```

`bFunctionalitySupport == 1` → Full Speed → floor `12`. Again informational.

### 7.3 A SuperSpeedPlus blob — the `usb_capable_gen` case

Real blob, captured from the `usb2` xHCI root hub on the reference machine
(`speed == 10000`, one lane each way):

`05 0f 2b 00 02 | 0a 10 03 02 08 00 01 00 00 00 | 1c 10 0a 00 23 00 00 00 04 11 00 00 34 00 05 00 b4 00 05 00 35 40 0a 00 b5 40 0a 00`

> **Caveat:** kernel-synthesised root hubs (`usb1`, `usb2`, …) are **not**
> published as `ListDevices` entries — `usbeehive` has always excluded them.
> The blob is quoted because it is the only SuperSpeedPlus descriptor
> available on this machine; the property values below are the decoder's
> output for it, i.e. what an attached SuperSpeedPlus *device* carrying this
> descriptor would publish.

```
link_speed_mbps = 10000
properties      = [
    ("usb_capable_speed_mbps",    "10000"),
    ("usb_capable_speed",         "SuperSpeed+ 10 Gbps"),
    ("usb_capable_gen",           "Gen 2x1"),
    ("usb_capable_rx_lanes",      "1"),
    ("usb_capable_tx_lanes",      "1"),
    ("usb_functional_floor_mbps", "5000"),
    ("usb_link_verdict",          "AtCapability"),
]
```

`usb_capable_gen == "Gen 2x1"` is the value that cannot be derived from
`link_speed_mbps` alone: a **Gen 1x2** device (two 5 Gbps lanes) also
aggregates to 10 000 Mbps and also reports `speed == 10000`. Only the
SuperSpeedPlus sublink array distinguishes them. If you show a generation
label, use this key — do not compute one from the speed.

### 7.4 `usb:5-2.1.2` — "Samsung Type-C Monitor", Billboard device

```
link_speed_mbps = 12
properties      = [
    ("usb_max_power_ma",   "100"),
    ("transport.usb2",     "true"),
    ("usb_altmode_svids",  "ff01"),
    ("usb_altmode_state",  "NotAttempted"),
    ("usb_altmode_failure","no_battery"),
]
```

SVID `ff01` is DisplayPort; the device reports that DP alt-mode configuration
was **never attempted**. Its BOS carries no speed capability, so `data_rate`
is `verdict: "Unknown"` and no `usb_capable_*` / `usb_link_*` key appears.

Billboard information is exposed as **facts only** — there is no warning flag
and no signal for it. A Billboard device reporting `NotAttempted` is very
often simply not connected through a Type-C port at all, so a client should
present it descriptively ("DisplayPort alt mode: not attempted") rather than
as a fault. `Unsuccessful` together with `usb_altmode_failure` containing
`no_usb_pd` is the genuinely actionable combination — that is a cable or port
that cannot do USB-PD.

### 7.5 A device with no BOS — the common case

E.g. "TP-Link UB500 Adapter", "MYSTIC LIGHT", "USB-PS/2 Optical Mouse" — 7 of
the 11 published entries on the reference machine:

```
properties     = [ ("serial", "…"), ("mount", "removable"),
                   ("usb_max_power_ma", "…"), ("transport.usb2", "true") ]
data_rate      = null
usb_device.bos = null
```

No `usb_capable_*`, no `usb_link_*`, no `usb_altmode_*`, no
`usb_bos_suppressed`. Correct, expected, silent.

### 7.6 Reference-machine summary

Of 11 published entries: 7 carry no BOS at all; 2 carry a BOS with a speed
capability (both `BelowCapability`, both correctly **not** warned about);
1 carries a Billboard-only BOS; 1 carries a BOS holding only a USB 2.0
Extension, i.e. no speed-bearing capability. **Zero** entries are `Degraded`. That is the intended outcome —
this machine has no misplugged device, and a naive
`capable > negotiated` comparison would have raised 2 false warnings.

---

## 8. Client integration checklist

1. Nothing is required. Existing behaviour is unaffected.
2. Add the 13 keys of §3 to `label-table.js` using the proposed display labels
   (they are the same strings the `usbeehive` CLI uses, so text stays
   consistent between the two front-ends).
3. Treat `usb_link_degraded` and `usb_bos_suppressed` as **flag keys** — render
   the label alone on presence; never render `: true`.
4. Treat unknown `usb_link_verdict` / `usb_altmode_state` values as unknown and
   render neutrally.
5. Only draw a warning for `Degraded`. Do not derive one from
   `usb_capable_speed_mbps > link_speed_mbps` (§6).
6. Optionally subscribe to `DataRateDegraded` / `DataRateRestored` for
   notifications. If you already re-snapshot on `DeviceChanged` you are
   correct without them — a verdict flip also fires `DeviceChanged`.
7. Never conclude "USB 2.0 only" from a missing BOS (§5.2).
8. `MIN_USBEEHIVE_VERSION` does **not** need to change. A client written
   against this document runs unmodified against an older daemon — every new
   key is simply absent, which is a state the client must handle anyway.
