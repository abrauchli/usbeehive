---
phase: quick-260917-mpq
plan: 01
subsystem: summary
tags: [usb-pd, cable-emarker, false-positive, release]
status: complete
requires: [260917-mdl]
provides: ["cable.no_emarker condition-identical to ChargingDiagnostic on both axes", "v0.12.1 release prepared locally"]
affects: [src/summary.rs, Cargo.toml, Cargo.lock, CHANGELOG.md]
tech_stack:
  added: []
  patterns: ["single active-PDO lookup feeding both contract power and hint basis"]
key_files:
  created: []
  modified:
    - src/summary.rs
    - Cargo.toml
    - Cargo.lock
    - CHANGELOG.md
decisions:
  - "Charger-side basis unified on the ACTIVE PDO current, closing ID-01"
  - "0.12.1 chosen as a PATCH bump — behaviour narrows, no interface change"
metrics:
  duration: ~18m
  completed: 2026-09-17
commits: 2
plan_head_before: ba93907
actuals:
  tokens: 21000
  tasks: 4
  commits: 2
---

# Quick 260917-mpq: Close ID-01 — cable.no_emarker on the active PDO Summary

The `cable.no_emarker` summary property now measures the charger side by the current the
**negotiated** PDO offers rather than the maximum advertised anywhere in the caps, making it
condition-identical to `ChargingDiagnostic::evaluate` on both axes; shipped as a local 0.12.1
patch release.

## What Changed

**Part A — the fix (`6f5990e`, `src/summary.rs` only).** The active-PDO lookup is hoisted into
a single `source_capabilities` scan feeding both the contract/display power figure and the
hint's new charger-side current, mirroring `src/diagnostic.rs:82-87`. The old
maximum-advertised binding was deleted outright.

### Final guard condition (verbatim, `src/summary.rs:1181-1188`)

```rust
if port.partner.is_some()
    && active_ma > 3_100
    && s.cable.as_ref().and_then(|c| c.current_rating).is_none()
    && (2_900..=3_100).contains(&requested_ma)
{
    s.properties
        .push(("cable.no_emarker".into(), "true".into()));
}
```

Fed by the single hoisted lookup (`src/summary.rs:1144-1148`):

```rust
let active_pdo = pd_port.source_capabilities.iter().find(|p| p.is_active);
let active_pdo_mw = active_pdo.map(|p| p.power_mw);
let active_ma = active_pdo.map(|p| p.current_ma).unwrap_or(0);
```

Condition identity is verified mechanically: `active_ma > 3_100` and `(2_900..=3_100)` each
return exactly two grep hits, one in `src/summary.rs`, one in `src/diagnostic.rs:102-103`.

**Residual non-numeric divergence — ORCHESTRATOR AUDIT NOTE, out of this task's scope.** Both
*numeric* axes are now identical, which is what ID-01 asked for. Two structural differences
remain and are deliberately untouched here:

1. The summary property additionally requires `port.partner.is_some()`; the diagnostic has no
   such conjunct. This only ever makes the property *stricter*, so it cannot produce a
   false positive.
2. `CableNoEMarker` is the `else if` arm after `CableLimit` in
   `ChargingDiagnostic::evaluate`, so a cable with `max_watts > 0 && max_watts < charger_max_w`
   but `current_rating == None` yields the `CableLimit` verdict while the summary property
   still emits `cable.no_emarker`. That is a pre-existing divergence on the *cable* axis,
   older than ID-01 and not part of it. Flagged for a future task; not fixed here.

**Part B — the release (`884b8a0`, `Cargo.toml` + `Cargo.lock` + `CHANGELOG.md`).** Version
0.12.0 → 0.12.1, `Cargo.lock` refreshed by exactly one line (matching the `dc7e352`
precedent), CHANGELOG `## [0.12.1] - 2026-09-17` section folding both fixes with an empty
`## [Unreleased]` retained above it and both link references updated.

## TDD Observations

**RED, recorded verbatim before any production change:**

```
test summary::tests::cable_no_emarker_property_absent_when_active_pdo_is_3a ... FAILED
thread '...' panicked at src/summary.rs:2500:9:
the device is drawing what its own active 3A PDO offers — that is not evidence of a 3A
cable pin, so the hint must not fire
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 211 filtered out
```

The failure was the first assertion (property present when it must be absent) — the correct
reason. After the production change: `9 passed; 0 failed`.

## Test Fixtures Changed

| Fixture | Change | Why |
|---------|--------|-----|
| `cable_no_emarker_property_absent_when_active_pdo_is_3a` | **NEW** | The originating bug report's layout: active 9V/3A PDO + merely-advertised 20V/5A, no cable node, 3A request. Asserts both the absent property and a non-`CableNoEMarker` verdict. |
| `cable_no_emarker_property_fires_for_big_charger_without_cable_node` | `is_active: true` added to its 20V/3.25A PDO; comment extended | Makes the charger-side basis explicit rather than implied by a distant inference pass. 3250mA > 3100, so the test still exercises exactly what its name says. See ID-01-a. |
| `cable_no_emarker_property_silent_for_3a_only_charger` | `is_active: true` added to its 5V/3A PDO; comment extended | So the reason in the test's name (3000 is not > 3100) is what keeps it silent, not the vacuous absence of an active PDO. See ID-01-c. |
| `cable_no_emarker_property_silent_when_emarker_rating_present` | untouched | The visible e-marker rating conjunct still carries it. |
| `cable_no_emarker_property_absent_when_5a_contract_negotiated` | untouched | Already `is_active: true`; still absent via the requested-current bounds. |

## Deviations from Plan

**1. [Rule 1 — plan prediction falsified] `..._fires_for_big_charger_without_cable_node` did
NOT go red as predicted.**

- **Found during:** Task 1 step 6 (the empirical reconciliation pass).
- **Predicted:** the fixture's PDO carries no `is_active`, so `active_ma` would be 0 and the
  hint would stop firing.
- **Observed:** the test passed unchanged. Root cause: `DeviceSummary::from_typec_port` runs
  `pd_port.infer_active_source_pdo(live_mv)` at `src/summary.rs:1086-1096` — *before* the
  guard — which marks the PDO active from the live `voltage_now_uv`. That fixture reports
  20V and its sole PDO is 20V, so it is inferred active and `active_ma` is 3250.
- **Action:** added `is_active: true` explicitly anyway (idempotent with the inference, which
  still runs and still matches) plus a comment naming the active-PDO basis. The plan's
  *direction* was right; only its stated cause was wrong. Recorded rather than absorbed, per
  the prior task's ID-02 discipline.
- **Why intent is preserved, not weakened:** the test's name promises "a big charger whose
  active PDO offers more than 3A, no cable node, request pinned at 3A ⇒ hint fires". 3250mA
  still exceeds the 3100mA threshold, the cable node is still absent, and the request is still
  pinned at 3.0A. Every conjunct it exercised before, it exercises now — the fixture merely
  states the active PDO rather than relying on inference to supply it.

No other deviations. No architectural changes, no auth gates, no checkpoints.

## Inferred Decisions — Marked for Audit

- **ID-01 (CLOSED)** — the charger-side basis is unified with the diagnostic's active-PDO
  current. The divergence left open by 260917-mdl is gone; the property and the
  `CableNoEMarker` verdict are now pinned on both axes.
- **ID-01-a** — `cable_no_emarker_property_fires_for_big_charger_without_cable_node` gains
  `is_active: true`. The empirical RED that the plan predicted did **not** materialize (see
  Deviations); the edit was made for legibility, and the test's intent is preserved in full.
- **ID-01-b** — a port where the kernel marks no PDO `is_active` no longer raises the hint at
  all. This is the exact objection ID-01 raised against unification, now deliberately
  accepted: `ChargingDiagnostic::evaluate` has always behaved this way, and condition-identity
  is the point. Documented in the guard's comment and stated explicitly in the CHANGELOG as a
  user-visible narrowing.
- **ID-01-c** — `cable_no_emarker_property_silent_for_3a_only_charger` gains `is_active: true`
  so the reason in its name is the operative one.
- **ID-03 (repeat)** — MSRV 1.85 build **SKIPPED**. `rustup toolchain list` shows stable,
  nightly, 1.87, 1.88, 1.88.0 — no 1.85 — and installing one is out of scope. The change
  introduces no new language or std feature, so MSRV risk is nil.
- **ID-04 (repeat)** — proceeded without ROADMAP/STATE/PROJECT; none exist in this repo.
- **ID-05 (repeat)** — committed directly on `master`, per `git.branching_strategy: "none"` in
  `.planning/config.json` and the repo's master-direct history.
- **ID-06** — 0.12.1 chosen as a **patch** bump: behaviour narrows for an existing advisory
  key, but no API, key name, D-Bus signature or interface version changes.

## CI Gate — Final Commit (`884b8a0`, bumped tree)

Every command run as `rtk proxy` and judged by **exit code** on raw, unfiltered output.

| Check | Exit | Observed result line |
|-------|------|----------------------|
| `cargo build --locked` | 0 | `Finished \`dev\` profile ... in 0.05s` |
| `cargo test --locked` | 0 | `test result: ok. 220 passed; 0 failed` (lib) + 7 further suites, all `0 failed` |
| `cargo build --locked --no-default-features` | 0 | `Compiling usbeehive v0.12.1` → `Finished` |
| `cargo test --locked --no-default-features` | 0 | `test result: ok. 170 passed; 0 failed` (lib) + 6 further suites, all `0 failed` |
| `cargo fmt --all --check` | 0 | (no output — clean) |
| `cargo clippy --all-targets -- -D warnings` | 0 | `Checking usbeehive v0.12.1` → `Finished` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | 0 | `Generated target/doc/usbeehive/index.html` |
| MSRV 1.85 | — | **SKIPPED** (ID-03 — toolchain not installed) |

No `allow` attribute was added anywhere to achieve clippy silence. The same gate was run and
was equally green on the fix commit `6f5990e` before the bump.

## Packaging Dry Run

1. **STRICT — `cargo publish --dry-run --locked`: exit 101.** Classified as a **dirty-tree
   refusal**, not a packaging defect. The crates.io index updated successfully (so this is
   also not a network failure). The five named files are all pre-existing untracked paths
   plus this task's own PLAN.md: `.gsd/dispatch-isolation-sentinel.json`,
   `.planning/config.json`, `.planning/quick/20260509-rename-to-usbeehive/SUMMARY.md`,
   `.planning/quick/260917-mpq-.../260917-mpq-PLAN.md`, `UI_PLAN.md`.
2. **`cargo publish --dry-run --locked --allow-dirty`: exit 0.** `Packaged 75 files, 917.4KiB
   (261.5KiB compressed)`, the packaged crate compiled clean under `Verifying`, and
   `aborting upload due to dry run`. **No real packaging defect** — no bad metadata, no
   missing README, no path dependency. Nothing was uploaded.

**Registry credentials:** `present` (existence probe only, via `test -f`; contents never read,
printed or quoted — threat T-260917-mpq-01).

## Release Readiness

| Item | State |
|------|-------|
| `6f5990e` | `fix(quick-260917-mpq): key cable.no_emarker on the active PDO current` — `src/summary.rs` only |
| `884b8a0` | `Release 0.12.1 — cable.no_emarker false-positive fixes` — `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml` only |
| Scope | `git diff --stat ba93907..HEAD` touches exactly those four files. `src/dbus.rs` diff is empty; no `src/output.rs`, no `.planning/specs/`, nothing under `../usbee`. |
| Interface | `cable.no_emarker` key string byte-identical; `org.usbeehive.Devices5` untouched. |
| Tags | Only `v0.12.0` exists. **No `v0.12.1` tag was created.** |

**Deliberately NOT done — OPERATOR-OWNED:**

- `git push` / `git push --tags` — **not run in any form.** (`git status --branch` reports
  `ahead 6`: four pre-existing unpushed commits plus these two.)
- `cargo publish` — **never run without `--dry-run`.**
- Creation of the annotated `v0.12.1` tag — **ORCHESTRATOR-OWNED**, deliberately left to the
  orchestrator after this executor returns so the tag is guaranteed to land on `884b8a0`.

## Known Stubs

None.

## Threat Flags

None. No new network endpoint, auth path, file-access pattern or schema change at a trust
boundary. The `PowerDataObject.current_ma` feeding the new guard is already `u32`
(`src/power.rs:50`), so no new cast is introduced, and the `.filter(|&ua| ua > 0)` protecting
`requested_ma` survives intact (verified by gate, threat T-260917-mpq-04).

## Self-Check: PASSED

- `src/summary.rs`, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` — all present and modified.
- Commits `6f5990e` and `884b8a0` — both confirmed present in `git log`.
- `git rev-list --count ba93907..HEAD` = 2, matching the `commits: 2` frontmatter claim.
