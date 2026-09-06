# D-Bus consumer spec — physical connector, power source, kernel quirks, database name

**Audience:** the author of a client that consumes `org.usbeehive.Devices5`
(e.g. the `usbee` GNOME Shell extension) and cannot see the daemon's source.

**Status:** implemented, unreleased. Present from the next `usbeehive`
release after 0.11.0. Ships alongside — and is designed to be read together
with — `DBUS-BOS-CONSUMER-SPEC.md`.

**Interface version: `org.usbeehive.Devices5` — UNCHANGED.**
Purely additive. There is no `Devices6`, no bus-name change, no object-path
change, no new signal, and no change to any existing method's, property's or
signal's signature. A client that ignores this entire document keeps working
byte-for-byte as before.

**Scope note.** This is a deliberately trimmed subset of the survey proposal
in `DBUS-FURTHER-SPEC.md`. That document lists ~30 keys; **11 shipped**.
Everything else there — `pm.*`, `connected_s`, `function.*`,
`interface.<N>.name`, `usb.configurations`, `hub.multi_tt`,
`power.remote_wakeup_capable`, `kernel.avoid_reset`, `authorized`,
`port.disabled`, and the `PortOverCurrent` signal — is **not emitted**. Do
not write a client against those.

---

## 1. What this adds, in one paragraph

`usbeehive` described devices; it did not describe the **connector they are
plugged into**, where their **power comes from**, or what the **kernel thinks
is wrong with them**. This adds all three, plus the hardware-database model
name for devices whose own `iProduct` string is empty or uninformative. The
single most valuable key is `port.peer_state`: it is the answer to "*why* is
this SuperSpeed-capable device linked at 480 Mbps" — see §6.

---

## 2. Wire-surface delta

### 2.1 Nothing changed

| Member | Signature | Change |
|---|---|---|
| `ListDevices` | `() → a(ssssssssssqqsa(ss)ius(uuus)(bsssb)a(usuuuub)i)` | **none** |
| `ListPorts`, `Diagnose`, `Refresh`, `Version`, `DeviceCount` | | none |
| `SnapshotJson` | `() → s` | JSON gains fields (§4); D-Bus signature unchanged |
| `DeviceAdded` `(ss)`, `DeviceRemoved` `(s)`, `DeviceChanged` `(s)` | | none |
| `CapabilityDegraded` `(iss)`, `CapabilityRestored` `(i)` | | none |
| `DataRateDegraded` `(sss)`, `DataRateRestored` `(s)` | | none |

The per-entry tuple shape is **not** touched. Positional unpackers need no
change. **No new signal was added.**

### 2.2 What did change

Eleven new keys in the per-entry `properties` bag (field 14, type `a(ss)`),
plus the `SnapshotJson` fields backing them (§4), plus three extra keys in
the `DeviceChanged` state fingerprint (§5).

---

## 3. New `properties` keys

`properties` is field 14 of a `ListDevices` entry, of type `a(ss)` —
`(machine_key, value)` pairs. Adding keys is explicitly documented as
non-breaking on this interface. All values are **UTF-8 strings**, including
the numeric ones.

These keys appear only on entries whose `category` is `UsbDevice` or `Hub`.
They **never** appear on `TypeCPort` entries — the connector story belongs to
the enumerated USB device, which has its own entry (correlated from the port
via the existing `usb_device` property).

**Every key below is optional.** Absence is normal and load-bearing; see §7.

### 3.1 Physical connector

| Machine key | Value format | Units | Proposed English display label |
|---|---|---|---|
| `port.id` | hub-port object name, e.g. `"usb5-port2"`, `"5-2.4-port1"` | — | Port |
| `port.peer_id` | companion hub-port object name, e.g. `"usb6-port2"` | — | Companion port |
| `port.peer_state` | kernel `state` verbatim (§3.5) | — | Companion port state |
| `port.connect_type` | `"hotplug"` \| `"hardwired"` \| `"not used"` | — | Connector |
| `hub.ports_total` | decimal integer, e.g. `"4"` | ports | Hub ports |
| `hub.ports_used` | decimal integer, e.g. `"2"` | ports | Hub ports in use |

### 3.2 Power source and budget

| Machine key | Value format | Units | Proposed English display label |
|---|---|---|---|
| `power.source` | `"bus"` \| `"self"` | — | Powered by |
| `hub.power_budget_ma` | decimal integer, `"500"` or `"900"` | mA | Hub budget (mA) |
| `hub.power_committed_ma` | decimal integer, e.g. `"188"` | mA | Hub committed (mA) |

### 3.3 Kernel

| Machine key | Value format | Units | Proposed English display label |
|---|---|---|---|
| `kernel.quirks` | comma-separated names, e.g. `"NO_LPM"`, `"RESET_RESUME,NO_BOS"` | — | Kernel workaround |

### 3.4 Hardware database

| Machine key | Value format | Units | Proposed English display label |
|---|---|---|---|
| `product_db` | free text, e.g. `"RTS5411 Hub"` | — | Model (USB database) |

That is **11 translatable strings** for `label-table.js` / `po/`. There are
no flag keys in this family — every key here carries a value, so all eleven
render as `label: value`. (Contrast the BOS family, which has two flag keys.)

### 3.5 `port.peer_state` value vocabulary

Passed through from the kernel verbatim, lowercase, and **may contain a
space**:

`not attached` · `attached` · `powered` · `reconnecting` ·
`unauthenticated` · `default` · `addressed` · `configured` · `suspended`

**New values may appear without an interface bump** (this is the interface's
standing enum-extensibility convention). Treat an unrecognised value as
unknown and render it neutrally, or render the raw string.

Only three are common in practice: `not attached` (nothing on that half of
the connector), `configured` (a device is enumerated there), `suspended` (a
device is there but asleep).

### 3.6 `kernel.quirks` value vocabulary

Comma-separated, ascending by bit position, with the kernel's `USB_QUIRK_`
prefix stripped. As of the Linux 7.0 snapshot the daemon ships:

`STRING_FETCH_255` · `RESET_RESUME` · `NO_SET_INTF` · `CONFIG_INTF_STRINGS` ·
`RESET` · `HONOR_BNUMINTERFACES` · `DELAY_INIT` ·
`LINEAR_UFRAME_INTR_BINTERVAL` · `DEVICE_QUALIFIER` · `IGNORE_REMOTE_WAKEUP` ·
`NO_LPM` · `LINEAR_FRAME_INTR_BINTERVAL` · `DISCONNECT_SUSPEND` ·
`DELAY_CTRL_MSG` · `HUB_SLOW_RESET` · `ENDPOINT_IGNORE` ·
`SHORT_SET_ADDRESS_REQ_TIMEOUT` · `NO_BOS` · `FORCE_ONE_CONFIG`

A bit the daemon's build does not know renders as `bit<N>` (e.g. `"bit19"`)
rather than being dropped, so a newer kernel is still reported. **Do not
translate these names** — they are kernel identifiers; put them in the value
position and translate only the label.

One name is cross-referenced by the BOS spec: `NO_BOS` means the kernel
deliberately never read this device's Binary Object Store, so a missing BOS
says nothing about capability. That case is already surfaced separately as
the flag key `usb_bos_suppressed` — see `DBUS-BOS-CONSUMER-SPEC.md` §5.2.

---

## 4. `SnapshotJson` additions

`SnapshotJson()` returns `serde_json` of the array of internal
`DeviceSummary` values. Eight new fields on `usb_device`, all additive with
serde defaults, in the same style as the `usb_device.bos` / `usb_device.quirks`
additions:

```json
"usb_device": {
  "port_id": "usb5-port2",
  "port_peer_id": "usb6-port2",
  "port_peer_state": "not attached",
  "port_connect_type": "hotplug",
  "max_child": 4,
  "ports_used": 2,
  "bm_attributes": 224,
  "product_db": "RTS5411 Hub"
}
```

| Field | Type | Meaning |
|---|---|---|
| `port_id` | string | `""` when the kernel publishes no `port` symlink |
| `port_peer_id` | string | `""` when the port has no companion |
| `port_peer_state` | string | `""` when there is no companion or its `state` was unreadable |
| `port_connect_type` | string | `""` when unreadable **or** when the kernel said `unknown` |
| `max_child` | integer | `0` for non-hubs and when `maxchild` was unreadable |
| `ports_used` | integer \| null | `null` when this is not a hub or no port object was readable. `0` is a real answer |
| `bm_attributes` | integer | Raw config `bmAttributes`. Bit 6 = self-powered, bit 5 = remote wakeup, bit 7 reserved-and-set — so `0` can only mean "not read" |
| `product_db` | string | Raw hwdb name, present here even when the `product_db` **property** was suppressed as redundant (§7.4) |

`bm_attributes` also carries bit 5, remote-wakeup capability, which this cut
deliberately does **not** surface as a property key. A client that wants it
today can read the bit from `SnapshotJson`; a future release may promote it.

The derived figures behind `hub.power_budget_ma` and
`hub.power_committed_ma` are not separate JSON fields — they are computed
from `bm_attributes`, `speed`, and the children's `max_power_ma`.

---

## 5. Effect on `DeviceChanged`

`DeviceChanged(id)` fires when a device's *curated* user-visible state
changes. Three of the new keys joined that fingerprint:

| Key | Fingerprinted | Why |
|---|---|---|
| `port.peer_state` | **yes** | A companion port training late (or dropping) is exactly the transition that flips a BOS verdict from "could go faster" to "is going faster". Consumers want to re-snapshot on it. |
| `power.source` | yes | Static per connection; free to include, catches a re-configuration. |
| `kernel.quirks` | yes | Static per connection. |
| `hub.ports_used` | **no** | Moves only on plug/unplug, which already fires `DeviceAdded` / `DeviceRemoved` for the device itself. Fingerprinting it would add a redundant `DeviceChanged` on the hub for the same event. |
| `hub.power_committed_ma` | **no** | Same reason. |
| `port.id`, `port.peer_id`, `port.connect_type`, `hub.ports_total`, `hub.power_budget_ma`, `product_db` | no | Static, and a change implies a re-enumeration under a new `id` anyway. |

**Nothing added here can make `DeviceChanged` chatter on an idle machine.**
Every value is either static for the life of a connection or moves only on a
real physical event. Verified on the reference machine: 60 consecutive
enumerations over 15 s of 11 live entries produced zero diffs of any kind.

**No new signal.** If you want to be notified when a companion port trains,
subscribe to `DeviceChanged` and re-read `port.peer_state`.

---

## 6. The pairing that makes the BOS verdict actionable

Read this together with `DBUS-BOS-CONSUMER-SPEC.md` §6.

The BOS family tells you a device is linked below what it advertises. It
cannot tell you **why**. `port.peer_state` can.

**Live proof on the reference machine.** Hub `usb:5-2` is a Realtek RTS5411
— a **USB 3** hub — enumerated at 480 Mb/s:

```
id                        = "usb:5-2"
category                  = "Hub"
link_speed_mbps           = 480
usb_capable_speed         = "SuperSpeed 5 Gbps"     ← BOS: could go faster
usb_link_verdict          = "BelowCapability"
port.id                   = "usb5-port2"
port.peer_id              = "usb6-port2"            ← the SuperSpeed half
port.peer_state           = "not attached"          ← …which never trained
port.connect_type         = "hotplug"
product_db                = "RTS5411 Hub"           ← iProduct only says
                                                      "4-Port USB 2.0 Hub"
```

The BOS says *"this device could go faster."* `port.peer_state ==
"not attached"` says *"because the SuperSpeed lanes of this connector never
came up"* — a USB 2.0-only cable, or a USB-A 2.0 receptacle. `product_db`
independently corroborates it: hwdb names the silicon `RTS5411 Hub`, which
is a USB 3.0 part, while the device's own `iProduct` string says
`4-Port USB 2.0 Hub`.

Recommended rendering of the combination:

| BOS verdict | `port.peer_state` | Suggested copy |
|---|---|---|
| `BelowCapability` or `Degraded` | `not attached` | "Connected through a USB 2.0-only cable or port — the high-speed lanes are not connected." Actionable: try a different cable or a blue/labelled port. |
| `BelowCapability` or `Degraded` | `configured` / `suspended` | The fast lanes *are* up but this device is not on them. Likelier a hub or host limitation than a cable. Describe, do not instruct. |
| `BelowCapability` or `Degraded` | absent | The port has no companion at all (a hub-internal port, or a controller with no separate SuperSpeed root hub). **Say nothing about cables** — you do not have the evidence. |
| absent (no BOS) | any | Say nothing. A device with no BOS makes no capability claim; a `not attached` companion is then just a fact about the connector. Live example: the TP-Link UB500 Bluetooth dongle on `usb1-port1` has `port.peer_state = "not attached"` and no BOS at all — it is a USB 2.0 device and nothing is wrong. |

Only `usb_link_degraded` (BOS family) warrants a warning badge. This family
adds **explanation**, never a new warning.

---

## 7. What is guaranteed absent, and when

**A missing key is information, not a failure.** Every one of the eleven can
be absent, and clients MUST render correctly with none of them.

### 7.1 `port.peer_id` / `port.peer_state` — hub-internal ports

Only **root-hub** ports have a companion: the kernel pairs the USB 2.0 half
and the USB 3.x half of one physical receptacle. A device plugged into a
downstream hub port (`port.id` like `5-2.4-port1`) has no companion, so both
keys are absent. This is the majority of entries on a machine with hubs — on
the reference box, 9 of 11.

Absence here means "no companion exists", **not** "the companion is down".

### 7.2 `port.connect_type` — the kernel's own `unknown`

The kernel writes the literal string `unknown` on every hub-internal port.
The daemon **drops** it rather than emitting a useless value, because
absence already means unknown throughout this interface. So
`port.connect_type` is present essentially only on root-hub ports.

Live: present on all five root-hub-attached entries (`hotplug` ×3,
`hardwired` ×2), absent on all six hub-attached ones.

### 7.3 `hub.*` — hubs only, and `0` is a real answer

- All four `hub.*` keys are absent on non-hub entries.
- `hub.ports_total` is absent when `maxchild` is unreadable.
- `hub.ports_used` is absent when **no port object was readable** — which is
  a different statement from `"0"`. `"0"` means "this hub has ports and none
  are occupied" (live: `usb:3-6`, an empty 4-port Genesys hub). Do not
  collapse the two.
- `hub.power_budget_ma` is absent for **self-powered** hubs: they draw from
  their own supply, so there is no bus-derived ceiling worth quoting. It is
  present only for bus-powered hubs — `"500"` on a USB 2 link, `"900"` on a
  SuperSpeed one.
- `hub.power_committed_ma` is absent when the hub has no enumerated
  children.

### 7.4 `product_db` — absent when it adds nothing

Present only when the udev hardware database has a model name **and** that
name is not already contained (case-insensitively) in the device's own
`iProduct` string. hwdb answers a bare `"Hub"` for a lot of generic silicon,
which is noise next to an `iProduct` of `"USB2.0 Hub"`; it answers
`"RTS5411 Hub"` next to `"4-Port USB 2.0 Hub"`, which names the actual chip
and is worth showing.

It is also absent when the daemon was built without the `watch` feature
(that is where the udev dependency lives) or the machine has no udev
hardware database.

`product_db` **never overrides** the positional `product` (field 10) or
`headline` (field 6). It is a secondary, advisory string. It can be stale or
wrong for re-badged PIDs — present it as "Model (USB database)", not as the
product's name.

### 7.5 `kernel.quirks` — absent means the kernel is happy

Present only when the kernel applied at least one workaround. On the
reference machine exactly one of 11 entries carries it. Absence is the
overwhelmingly common, healthy case — **never render "no quirks"**.

### 7.6 `power.source` — absent means the descriptor was not read

Present on every normally-enumerated device (bit 7 of `bmAttributes` is
reserved-and-always-set, so a valid descriptor can never read as absent).
Absent only when the attribute could not be read at all.

### 7.7 Type-C port entries

Entries with `category == "TypeCPort"` carry **none** of these keys. For a
port's connector story, follow the existing `usb_device` property on the
port entry to the corresponding `usb:<bus_port>` entry.

---

## 8. Worked examples — real devices

All values captured live from the development machine (Ubuntu 26.04, kernel
7.0.0-31, AMD desktop, no Type-C hardware).

### 8.1 `usb:5-2` — RTS5411 USB 3 hub linked at 480 Mb/s

The headline case; see §6 for the full reading.

```
category   = "Hub"
properties = [ …,
  ("port.id",                "usb5-port2"),
  ("port.peer_id",           "usb6-port2"),
  ("port.peer_state",        "not attached"),
  ("port.connect_type",      "hotplug"),
  ("hub.ports_total",        "4"),
  ("hub.ports_used",         "2"),
  ("power.source",           "self"),
  ("hub.power_committed_ma", "100"),
  ("product_db",             "RTS5411 Hub"),
]
```

Absent: `hub.power_budget_ma` (self-powered), `kernel.quirks` (none).
`hub.power_committed_ma` is 100, not 200, because one of its two children is
itself a self-powered hub and contributes zero.

### 8.2 `usb:5-2.4` — bus-powered Dell keyboard hub

The power-budget case.

```
category   = "Hub"
properties = [ …,
  ("port.id",                "5-2-port4"),
  ("hub.ports_total",        "3"),
  ("hub.ports_used",         "2"),
  ("power.source",           "bus"),
  ("hub.power_budget_ma",    "500"),
  ("hub.power_committed_ma", "188"),
]
```

188 mA = the keyboard's declared 90 + the mouse's declared 98. Well inside
the 500 mA budget. Absent: everything `peer`-related and
`port.connect_type` — it hangs off a hub-internal port.

**Wording matters:** `bMaxPower` is a *declared maximum*, not a measured
draw. Say "declares", "committed", or "budgeted"; never "draws" or "using".

### 8.3 `usb:5-2.1.1` — RTL8153 gigabit adapter with a kernel workaround

```
category   = "UsbDevice"
link_speed_mbps = 480
properties = [ …,
  ("usb_capable_speed",  "SuperSpeed 5 Gbps"),   ← BOS family
  ("usb_link_verdict",   "BelowCapability"),     ← BOS family
  ("port.id",            "5-2.1-port1"),
  ("power.source",       "bus"),
  ("kernel.quirks",      "NO_LPM"),
  ("product_db",         "RTL8153 Gigabit Ethernet Adapter"),
]
```

`NO_LPM` — the kernel disabled USB link power management for this device
because its quirk table says the device mishandles it. Render as "Kernel
workaround: NO_LPM" in a technical-details section; it is an explanation, not
a fault. Note it is **not** `NO_BOS`, so the BOS verdict above stands.

No `port.peer_*` here: this device is two hubs deep.

### 8.4 `usb:1-4` — Intel Bluetooth with no `iProduct` at all

```
category   = "UsbDevice"
product    = ""                       ← the device publishes no iProduct
properties = [ …,
  ("port.id",           "usb1-port4"),
  ("port.connect_type", "hotplug"),
  ("power.source",      "self"),
  ("product_db",        "AX200 Bluetooth"),
]
```

Without `product_db` the only name available for this device is
`8087:0029`. Absent: `port.peer_id` — this root-hub port has no companion
(that connector is USB 2.0-only on this board).

### 8.5 `usb:3-6` — an empty hub

```
category   = "Hub"
properties = [ …,
  ("port.id",           "usb3-port6"),
  ("port.connect_type", "hardwired"),
  ("hub.ports_total",   "4"),
  ("hub.ports_used",    "0"),          ← a real answer, not "unknown"
  ("power.source",      "self"),
]
```

`hardwired` means this hub is soldered to the board, not user-pluggable — a
useful cue that "unplug and try another port" is not advice the user can
follow. Absent `product_db`: hwdb answers `"Hub"`, which the device's own
`"USB2.0 Hub"` already contains.

### 8.6 Reference-machine summary

Of 11 published entries: **11** carry `port.id` and `power.source`, 5 carry
`port.connect_type`, **2** carry `port.peer_id` / `port.peer_state`, 4 carry
the `hub.*` family, 1 carries `kernel.quirks`, 4 carry `product_db`. Zero
new warnings are raised by this family — by design.

---

## 9. Client integration checklist

1. Nothing is required. Existing behaviour is unaffected.
2. Add the 11 keys of §3 to `label-table.js` with the proposed display
   labels (they are the same strings the `usbeehive` CLI uses, so text stays
   consistent between the two front-ends). None of them is a flag key —
   render every one as `label: value`.
3. Do not translate the *values* of `port.peer_state` (kernel vocabulary) or
   `kernel.quirks` (kernel identifiers). Translate the labels only.
4. Treat an unrecognised `port.peer_state` value as unknown and render it
   neutrally; new values may appear without an interface bump.
5. Pair `port.peer_state` with the BOS verdict per §6 before saying anything
   about cables. Without a BOS verdict, a `not attached` companion is not a
   problem.
6. Distinguish absent `hub.ports_used` from `"0"` (§7.3).
7. Word every `hub.power_*` and `usb_max_power_ma` figure as *declared*, not
   *drawn*.
8. Do not draw a warning from anything in this family. It explains; the BOS
   family's `usb_link_degraded` and the charging family's
   `CapabilityDegraded` are the only warning sources.
9. `MIN_USBEEHIVE_VERSION` does **not** need to change. A client written
   against this document runs unmodified against an older daemon — every new
   key is simply absent, which is a state the client must handle anyway.
10. Do not implement anything from `DBUS-FURTHER-SPEC.md` that is not in
    this document; the daemon does not emit it.
