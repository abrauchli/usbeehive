---
phase: quick-260917-mdl
plan: 01
subsystem: summary
status: complete
tags: [bugfix, usb-c, power-delivery, false-positive, tdd]
requires:
  - "src/diagnostic.rs:103 — the canonical (2_900..=3_100) requested-current bounds"
provides:
  - "cable.no_emarker property guarded on the negotiated RDO operating current"
affects:
  - "org.usbeehive.Devices5 consumers that read cable.no_emarker (firing condition narrowed; key name unchanged)"
tech-stack:
  added: []
  patterns:
    - "Summary property firing conditions mirror the ChargingDiagnostic verdict bounds byte-for-byte so the two cannot disagree"
key-files:
  created: []
  modified:
    - src/summary.rs
    - CHANGELOG.md
decisions:
  - "Guard cable.no_emarker on requested_ma in (2_900..=3_100), byte-identical to src/diagnostic.rs:103"
  - "Charger-side basis left as max_advertised_ma > 3_000 rather than unified with the diagnostic's active_ma > 3_100 (ID-01, open audit item)"
  - "cable_no_emarker_property_fires_for_big_charger_without_cable_node gains a 3A-pinned power_supply fixture (ID-02)"
metrics:
  duration: ~12m
  completed: 2026-09-17
  tasks: 3
  commits: 2
  plan_head_before: 716e62fccd60238506d2e8a4a340d9f6d743056f
actuals:
  tokens: 36176   # chars/4 over the two changed files (same basis as the plan's raw_tokens 30000); the realized diff alone is 7114 chars ≈ 1779
  tasks: 3
  commits: 2      # MEASURED: git rev-list --count 716e62f..HEAD
---

# Quick 260917-mdl: Fix false-positive `cable.no_emarker` summary property — Summary

`cable.no_emarker` now additionally requires the RDO operating current to sit at the
spec's 3A default, so a UCSI laptop running a live 20V/5A 100W contract with no cable
node no longer reports a hint that its own payload contradicts.

## What Changed

**Root cause.** The property consulted only what the charger *advertised*
(`max_advertised_ma > 3_000`) and whether a cable e-marker `current_rating` was
*visible* — never what was actually *negotiated*. On a UCSI system whose PPM does not
answer `GET_CABLE_PROPERTY` the kernel registers no `/sys/class/typec/portN-cable/`
node at all, so `current_rating` is permanently `None` and **no cable, however good,
could ever satisfy that conjunct away**. The hint therefore fired on every >3A charger,
including ports carrying a 20V/5A contract that is itself proof of a 5A e-marked cable
(5A is only legal over one). `ChargingDiagnostic::evaluate` had always been correct here;
the property was the outlier.

**The fix** is a fourth conjunct at the push site (`src/summary.rs:1176-1181`), requiring
positive 3A-pin evidence rather than treating an absent rating as evidence on its own:

```rust
let max_advertised_ma = pd_port
    .source_capabilities
    .iter()
    .map(|p| p.current_ma)
    .max()
    .unwrap_or(0);
if port.partner.is_some()
    && max_advertised_ma > 3_000
    && s.cable.as_ref().and_then(|c| c.current_rating).is_none()
    && (2_900..=3_100).contains(&requested_ma)
{
    s.properties
        .push(("cable.no_emarker".into(), "true".into()));
}
```

The `(2_900..=3_100)` bounds are byte-identical to `src/diagnostic.rs:103`, verified by
`grep -rn '2_900..=3_100' src/` returning exactly those two lines. `requested_ma`'s
existing `.filter(|&ua| ua > 0)` (`src/summary.rs:1127`) is untouched, so a negative
sysfs `current_now` still cannot reach the `as u32` cast (threat T-260917-mdl-02).

The explanatory comment above the `if` was extended to record why an absent rating cannot
carry the hint alone, that an unknown (`0`) or higher request is not evidence, and that
the bounds are deliberately pinned to `ChargingDiagnostic::evaluate`.

## TDD Gate Compliance

| Gate | Evidence |
|------|----------|
| RED | `cable_no_emarker_property_absent_when_5a_contract_negotiated` written and run **before** the production change. Observed: `test result: FAILED. 0 passed; 1 failed`, panicking at the property assertion — "a negotiated 5A contract proves an e-marked cable — the hint must not fire". Intentional RED, correct reason (property present when it must be absent). |
| GREEN | Guard added; the same test passes. `test result: ok. 219 passed; 0 failed` on the lib suite. |
| REFACTOR | None required — the change is a single conjunct plus comment. |

Note: the RED run's *second* assertion (`charging_diag != Bottleneck::CableNoEMarker`)
did **not** fail, confirming the diagnostic was already correct and only the property
had drifted. That assertion is retained to pin the two together against future drift.

## Pre-existing Tests Touched (ID-02)

All three pre-existing `cable_no_emarker_*` summary tests were run after the guard landed,
and only the predicted one went red. None were pre-emptively edited.

| Test | Line (pre-change) | Outcome | Action |
|------|------|---------|--------|
| `cable_no_emarker_property_fires_for_big_charger_without_cable_node` | src/summary.rs:2304 | **FAILED** — its `TypeCPort` used `..Default::default()`, so `power_supply` was `None` and `requested_ma == 0` | Fixture gains `power_supply: Some(TypeCPowerSupply { online: true, voltage_now_uv: Some(20_000_000), current_now_ua: Some(3_000_000), .. })` — a real 3A-pinned contract on the 65W charger. Comment extended to say the 3A pin is now the evidence the hint keys on. Intent (big charger + no cable node ⇒ hint fires) preserved, not weakened. |
| `cable_no_emarker_property_silent_for_3a_only_charger` | src/summary.rs:2333 | **passed** untouched (fails the `max_advertised_ma > 3_000` conjunct first) | none |
| `cable_no_emarker_property_silent_when_emarker_rating_present` | src/summary.rs:2358 | **passed** untouched (fails the `current_rating().is_none()` conjunct first) | none |

`cable_no_emarker` test count is now 8 (4 in `summary`, 4 in `diagnostic`), up from 7.

## CI Gate Results (per check)

Every command run as `rtk proxy cargo ...` on raw, un-filtered output — judged by exit
code **and** by reading the actual result line, never by a grep for `test result:`
(which `rtk`'s filter would have made vacuous).

| Check | Exit | Observed result |
|-------|------|-----------------|
| `cargo build --locked` | 0 | `Finished \`dev\` profile` |
| `cargo test --locked` (default features) | 0 | `test result: ok. 219 passed; 0 failed` (lib) plus 12 / 7 / 5 / 18 / 12 passed across the integration binaries, 0 failed anywhere |
| `cargo build --locked --no-default-features` | 0 | clean |
| `cargo test --locked --no-default-features` | 0 | `test result: ok. 169 passed; 0 failed` (lib) plus `7 passed; 0 failed`, 0 failed anywhere |
| `cargo fmt --all --check` | 0 | no output (no diff) |
| `cargo clippy --all-targets -- -D warnings` | 0 | `Finished \`dev\` profile` — no warnings, and **no `allow` attribute was added** to achieve it |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | 0 | `Generated target/doc/usbeehive/index.html` — no rustdoc warnings |
| MSRV 1.85 build | **SKIPPED** | `rustup toolchain list` shows stable, nightly, 1.87, 1.88, 1.88.0 — no 1.85, and installing one is out of scope (ID-03) |

Scope check: `git diff --stat 716e62f..HEAD` touches exactly `CHANGELOG.md` (+25) and
`src/summary.rs` (+78/−3). No `src/dbus.rs`, no `Cargo.toml`, no `.planning/specs/`,
nothing under `../usbee`. `grep -rn 'cable.no_emarker' src/` confirms the key string is
byte-identical, so `org.usbeehive.Devices5` stays intact and the ../usbee consumer
(`property-policy.js:130`, `label-table.js:52,173`) keeps matching. No version bump.

**No pre-existing CI failures were encountered** — the gate was green on every check, so
there is nothing to record as predating the patch.

## Inferred Decisions — Marked for Audit

The human operator was unavailable; these were decided from the plan and the code.

- **ID-01 — Charger-side condition left as-is (`max_advertised_ma > 3_000`).** *Open audit
  item.* The brief scopes the fix to the requested/negotiated-current axis. Switching the
  charger-side basis to the diagnostic's `active_ma > 3_100` would change behavior beyond
  the reported bug: ports where no PDO is marked `is_active` would silently lose the hint
  entirely. **Residual divergence accepted and recorded:** when the active PDO is ≤3.1A but
  some *other* advertised PDO exceeds 3A, the property can still fire while the diagnostic
  does *not* emit `Bottleneck::CableNoEMarker`. Decide later whether to unify the
  charger-side basis too.
- **ID-02 — The `:2304` fixture change was legitimate, and was confirmed empirically**
  (run first, edited only after it actually went red, for exactly the predicted reason).
  See the table above.
- **ID-03 — MSRV 1.85 build skipped.** No 1.85 toolchain installed; installing one was
  forbidden. The change introduces no new language or std feature —
  `RangeInclusive::contains` is stable since 1.35 — so MSRV risk is nil.
- **ID-04 — Proceeded without ROADMAP/STATE/PROJECT**, per the dispatch note; this repo
  has run prior quick tasks without them. No state files were created or updated.
- **ID-05 (new, added during execution) — Committed directly on `master`.** The executor's
  default protocol halts on a commit to the default branch. Overridden here because the
  dispatch explicitly resolved isolation to `none` and instructed master-direct commits,
  `.planning/config.json` sets `git.branching_strategy: "none"`, and the repo's recent
  history is master-direct. Recorded rather than silently assumed.

## Known Stubs

None. No placeholder, TODO, or unwired code path was introduced.

## Threat Flags

None. No new sysfs field is read, no new property key is emitted, and the change strictly
narrows when an existing advisory string appears. The `<threat_model>` mitigations hold:
T-260917-mdl-02's `.filter(|&ua| ua > 0)` survives intact, and T-260917-mdl-03's key
string plus `src/dbus.rs` are untouched.

## Commits

| Hash | Message |
|------|---------|
| `b34485f` | `fix(quick-260917-mdl): require a 3A-pinned request for cable.no_emarker` |
| `e53ab41` | `docs(quick-260917-mdl): changelog entry for the cable.no_emarker fix` |

Task 3 modified no files (verification only, all checks green on first run), so it
produced no commit.

## Self-Check: PASSED

- `src/summary.rs` exists and contains the guard at line 1180 — verified by `grep`.
- `CHANGELOG.md` `### Fixed` under `## [Unreleased]` at line 10 — verified by `grep -n -A4`.
- Commits `b34485f` and `e53ab41` exist — verified by `git log --oneline 716e62f..HEAD`.
- `git rev-list --count 716e62f..HEAD` returns `2`, matching the `commits:` frontmatter.
- Working tree clean of tracked changes; the four untracked paths (`.gsd/`,
  `.planning/config.json`, `.planning/quick/20260509-rename-to-usbeehive/`, `UI_PLAN.md`)
  pre-date this task and were left alone.
