---
phase: quick-260917-mpq
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/summary.rs
  - Cargo.toml
  - Cargo.lock
  - CHANGELOG.md
autonomous: true
requirements: [QUICK-260917-mpq]

estimate:
  tokens: 34000
  raw_tokens: 34000
  tasks: 4
  confidence: low   # gsd estimate-calibration: sample_count 0, factor 1.0 (not applied)

must_haves:
  truths:
    - "A port whose ACTIVE PDO is 9V/3A, with a 20V/5A PDO merely advertised alongside it, no cable node and a 3A request, does NOT report `cable.no_emarker`."
    - "A port whose ACTIVE PDO offers more than 3.1A, with no cable node and a request pinned at ~3A, still DOES report `cable.no_emarker`."
    - "The summary property's firing condition is condition-identical to the `CableNoEMarker` branch of `ChargingDiagnostic::evaluate` on BOTH axes — charger-side current and requested current."
    - "`src/summary.rs` scans `source_capabilities` for the active PDO exactly once; the active PDO power and the active PDO current both derive from that single lookup."
    - "The `cable.no_emarker` key string, `src/dbus.rs` and the `org.usbeehive.Devices5` interface name are byte-unchanged."
    - "The full local CI gate (build + test for default AND --no-default-features, fmt, clippy, rustdoc) is green, judged by EXIT CODE on raw `rtk proxy` output."
    - "The crate is at version 0.12.1 with a `## [0.12.1] - 2026-09-17` CHANGELOG section and matching link references, committed locally — NOT pushed, NOT published, NOT tagged."
  artifacts:
    - "src/summary.rs — active-PDO charger-side basis, hoisted single `active_pdo` lookup, new regression test, two reconciled `cable_no_emarker_*` fixtures."
    - "Cargo.toml + Cargo.lock — version 0.12.1, lock in sync so `--locked` succeeds."
    - "CHANGELOG.md — `## [0.12.1] - 2026-09-17` section folding both fixes, empty `## [Unreleased]` retained, `[0.12.1]` link ref added, `[Unreleased]` compare range updated."
  key_links:
    - "The single hoisted `active_pdo` binding must feed BOTH `active_pdo_mw` (contract/display figure) and the new active current (hint basis) — a second `source_capabilities` scan is a defect."
    - "The summary guard's `active_ma > 3_100` must be byte-identical to the comparison at `src/diagnostic.rs:102`, and the `(2_900..=3_100)` bounds byte-identical to `src/diagnostic.rs:103`."
    - "`requested_ma`'s existing `.filter(|&ua| ua > 0)` at `src/summary.rs:1127` must survive — it is what keeps a negative sysfs `current_now` out of the `as u32` cast."
    - "`Cargo.lock`'s `usbeehive` version entry must track `Cargo.toml` or EVERY `--locked` command in the CI gate fails."
---

<objective>
Close inferred decision **ID-01** from quick task 260917-mdl: make the `cable.no_emarker`
summary property condition-identical to the `Bottleneck::CableNoEMarker` diagnostic, then
prepare the 0.12.1 patch release locally.

The residual false positive: the property still gates on `max_advertised_ma > 3_000` (the
MAXIMUM current advertised anywhere in the charger's caps, `src/summary.rs:1171-1178`) while
the diagnostic gates on the ACTIVE PDO's `active_ma > 3_100` (`src/diagnostic.rs:87,102`).
A charger advertising 5V/3A, 9V/3A and 20V/5A, with the device negotiating the **9V/3A** PDO,
therefore still trips the property (max advertised = 5000 > 3000, request 3000 in range, no
cable node) while the diagnostic correctly stays silent. The device is simply drawing what
its active 3A PDO offers — that is not evidence of a 3A cable pin. This is the exact PDO
layout from the originating bug report.

Purpose: one basis, one verdict. After this change the property and the diagnostic cannot
disagree on either axis, which is the invariant the previous task established and left half
finished.

Output: active-PDO-based guard + regression test in `src/summary.rs` (one commit), then a
0.12.1 version bump and CHANGELOG release section (a second commit). Nothing leaves the
machine.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@src/summary.rs
@src/diagnostic.rs
@CHANGELOG.md
@Cargo.toml

This repo has **no** `.planning/PROJECT.md`, `.planning/ROADMAP.md` and no
`.planning/STATE.md`, and none are needed — do not look for them and do not create them.
Ten prior quick tasks ran the same way. No project `CLAUDE.md`, no `.claude/skills/`.

`.planning/config.json` sets `git.branching_strategy: "none"` and the repo's recent history
is master-direct; commit directly on `master` (this repeats ID-05 from the prior task).

Prior task's summary, for the ID-01 statement being closed:
`.planning/quick/260917-mdl-fix-false-positive-cable-no-emarker-summ/260917-mdl-SUMMARY.md`
</context>

<critical_tooling_caveat>
**This environment wraps bash commands in `rtk`, which STRIPS lines matching `warning:` and
`test result:` from cargo output.** A grep for either string therefore succeeds VACUOUSLY and
produces a FALSE GREEN.

Every cargo / verification command in this plan MUST be invoked as `rtk proxy <cmd>` so the
raw, un-filtered output is visible, and MUST be judged by **EXIT CODE** plus by reading the
actual result line in the raw output. Never conclude "green" from a grep for `test result:`
or from the absence of `warning:`.
</critical_tooling_caveat>

<user_unavailable>
The human operator is UNAVAILABLE. Decide from the artifacts, record every inferred decision
under an `ID-NN` heading in the SUMMARY. There are no interactive checkpoints in this plan.
</user_unavailable>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Key cable.no_emarker on the ACTIVE PDO current (closes ID-01)</name>
  <files>src/summary.rs</files>
  <read_first>
    `src/summary.rs:1131-1192` (the `if let Some(pd_port)` block: `active_pdo_mw` at ~1144,
    the comment block at ~1152-1170, the deleted binding at ~1171-1176, the `if` at
    ~1177-1184) and `src/diagnostic.rs:82-104` (the canonical `active_pdo` / `active_ma`
    lookup and the `CableNoEMarker` branch this must mirror). Line numbers are from planning
    time — re-verify before editing.
  </read_first>
  <behavior>
    RED first. Add the new regression test BEFORE touching production code and observe it
    fail for the right reason (the property is present when it must be absent).

    - New test `cable_no_emarker_property_absent_when_active_pdo_is_3a`, placed immediately
      after `cable_no_emarker_property_absent_when_5a_contract_negotiated` (~L2456), in the
      style of its neighbours (doc comment explaining the scenario, then fixture, then
      assertions).
    - Fixture: `TypeCPort` with `partner: Some(TypeCPartner::default())` and
      `power_supply: Some(TypeCPowerSupply { online: true, voltage_now_uv: Some(9_000_000),
      current_now_ua: Some(3_000_000), ..Default::default() })` — i.e. `requested_ma` 3000.
    - `PowerDeliveryPort.source_capabilities` holds TWO fixed-supply PDOs: 9V/3000mA/27000mW
      with `is_active: true`, and 20V/5000mA/100000mW left NOT active.
      `max_source_power_mw: 100_000`. No `CableInfo` (no cable node at all).
    - Assert 1: no `cable.no_emarker` property, with a message saying the device is drawing
      what its own active 3A PDO offers — that is not evidence of a cable pin.
    - Assert 2 (pins property and verdict together, mirroring the neighbour test):
      `s.charging_diag`'s bottleneck is not `Bottleneck::CableNoEMarker`. (It will be
      `DeviceLimit` — 27W of a 100W charger — which is the correct verdict here.)
  </behavior>
  <action>
Run the new test first and record the RED observation verbatim for the SUMMARY, then make
the production change.

1. Hoist the active-PDO lookup. The block currently binds `active_pdo_mw` directly from a
   `pd_port.source_capabilities.iter().find(...)` chain. Replace that with a single
   `let active_pdo = pd_port.source_capabilities.iter().find(|p| p.is_active);` and derive
   BOTH `let active_pdo_mw = active_pdo.map(|p| p.power_mw);` and the new active current
   `let active_ma = active_pdo.map(|p| p.current_ma).unwrap_or(0);` from it. This mirrors
   `src/diagnostic.rs:82-87` exactly. Do NOT add a second scan of `source_capabilities` —
   one lookup feeds both. `contract_mw` and `sink_power_mw` keep consuming `active_pdo_mw`
   unchanged; `current_ma` is already `u32` (`src/power.rs:50`), so no cast is introduced.

2. Change the second conjunct of the `cable.no_emarker` `if` from the max-advertised
   comparison to `active_ma > 3_100`, byte-identical to the comparison at
   `src/diagnostic.rs:102`. The other three conjuncts (`port.partner.is_some()`, the
   `current_rating().is_none()` check, and `(2_900..=3_100).contains(&requested_ma)`) are
   untouched.

3. DELETE the now-unused max-advertised binding (the five-line
   `.iter().map(|p| p.current_ma).max().unwrap_or(0)` chain) entirely.
   <!-- planner-discipline-allow: max_advertised_ma -->
   **Do not name that removed identifier in any comment you write** — the verify gate greps
   non-comment lines for it, and naming it in prose would be merely confusing rather than
   failing, but the code must not resurrect it.

4. Rewrite the explanatory comment block above the `if` so it describes the ACTIVE-PDO basis
   instead of the max-advertised one: the hint keys on the current the *selected* PDO offers,
   because a 5A PDO that is merely advertised and not negotiated says nothing about the
   cable — a device on a 9V/3A contract is drawing exactly what it asked for. Keep the
   existing "held byte-identical to `ChargingDiagnostic::evaluate`" claim and make it
   TRUTHFUL for its now-wider meaning: both the charger-side threshold and the
   requested-current bounds are pinned to that function, so the property and the
   `CableNoEMarker` verdict cannot disagree. Keep the existing "not visible, never missing"
   framing about absent cable nodes.

5. Record in the comment the accepted consequence (this is the objection ID-01 raised, now
   deliberately accepted): a port where NO PDO is marked `is_active` yields 0 and therefore
   never raises the hint. `ChargingDiagnostic::evaluate` has always behaved exactly that way;
   aligning with it IS the point of this change. Log it as **ID-01-b** in the SUMMARY.

6. Reconcile the existing fixtures — EMPIRICALLY, in the prior task's ID-02 discipline: run
   the suite after the production change, observe which tests actually go red and why, and
   only then edit. Predictions made at planning time, to be confirmed not assumed:
   - `cable_no_emarker_property_fires_for_big_charger_without_cable_node` (~L2319) —
     PREDICTED RED: its single 20V/3.25A PDO carries no `is_active`, so the active current is
     0 and the hint stops firing. Fix by adding `is_active: true` to that PDO. 3250mA is
     greater than 3100, so the test still exercises precisely what its name says: a big
     charger whose ACTIVE PDO offers more than 3A, no cable node, request pinned at 3A ⇒ hint
     fires. Extend its comment to say the active PDO is now the charger-side basis. Record as
     **ID-01-a** (which test, and why the change preserves rather than weakens its intent).
   - `cable_no_emarker_property_silent_for_3a_only_charger` (~L2357) — PREDICTED still green,
     but now for the WRONG reason (no active PDO rather than a charger that never exceeds 3A).
     Add `is_active: true` to its 5V/3A PDO so the named reason is what keeps it silent (3000
     is not greater than 3100). Record as **ID-01-c**.
   - `cable_no_emarker_property_silent_when_emarker_rating_present` (~L2382) — leave alone;
     the visible e-marker rating conjunct still carries it.
   - `cable_no_emarker_property_absent_when_5a_contract_negotiated` (~L2411) — leave alone;
     already `is_active: true`, still absent via the requested-current bounds.

Make NO commit in this task — Task 2 commits Part A as one atomic commit after the gate.
Do not touch `src/dbus.rs`, `src/output.rs`, `.planning/specs/`, `Cargo.toml`, or anything
under `../usbee`.
  </action>
  <verify>
    <automated>rtk proxy cargo test --locked cable_no_emarker</automated>
    <automated>rtk proxy sh -c "grep -v '^[[:space:]]*//' src/summary.rs | grep -c 'active_ma > 3_100' | grep -qx 1"</automated>
    <automated>rtk proxy sh -c "grep -v '^[[:space:]]*//' src/summary.rs | grep -c 'max_advertised_ma' | grep -qx 0"</automated>
    <automated>rtk proxy sh -c "grep -Fc 'find(|p| p.is_active)' src/summary.rs | grep -qx 1"</automated>
    <automated>rtk proxy sh -c "grep -Fc 'filter(|&ua| ua > 0)' src/summary.rs | grep -qx 1"</automated>
    <automated>rtk proxy git diff --name-only HEAD</automated>
  </verify>
  <done>
    Exit code 0 from the targeted test run with the raw output showing the new test plus all
    four pre-existing `cable_no_emarker_*` summary tests and all four diagnostic ones passing,
    0 failed. Exactly one non-comment occurrence of the active-current threshold, zero
    non-comment occurrences of the removed max-advertised binding, exactly one
    `source_capabilities` active-PDO scan in `src/summary.rs`, and the negative-current filter
    intact. `git diff --name-only` lists `src/summary.rs` and nothing else. The RED observation
    for the new test, and the observed failure of the `..._fires_...` fixture, are captured
    verbatim for the SUMMARY.
  </done>
</task>

<task type="auto">
  <name>Task 2: Full local CI gate, then commit Part A</name>
  <files>src/summary.rs</files>
  <action>
Re-run the entire gate from `.github/workflows/ci.yml` locally. EVERY command as
`rtk proxy ...`, judged by exit code and by reading the raw result lines:

- `rtk proxy cargo build --locked`
- `rtk proxy cargo test --locked`
- `rtk proxy cargo build --locked --no-default-features`
- `rtk proxy cargo test --locked --no-default-features`
- `rtk proxy cargo fmt --all --check`
- `rtk proxy cargo clippy --all-targets -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps`

MSRV 1.85 is in CI but no 1.85 toolchain is installed and installing one is out of scope —
SKIP it and record the skip (this repeats ID-03; the change introduces no new language or
std feature, so MSRV risk is nil). Confirm the skip with `rtk proxy rustup toolchain list`.

If clippy or fmt demands a change, make it — but never silence clippy with an `allow`
attribute; fix the code. If a check fails for a reason that PREDATES this task, record that
explicitly rather than absorbing it.

Then commit Part A as ONE atomic commit containing `src/summary.rs` only:

  `fix(quick-260917-mpq): key cable.no_emarker on the active PDO current`

Body: the property gated on the maximum current advertised anywhere in the charger's caps
while `ChargingDiagnostic::evaluate` gated on the ACTIVE PDO's current, so a device on a
9V/3A contract from a charger that also advertises 20V/5A still tripped the hint while the
diagnostic stayed silent. Both axes are now pinned to the diagnostic. Note the accepted
consequence (no active PDO ⇒ no hint, matching the diagnostic). Closes ID-01 from 260917-mdl.
End the message with the `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>` line.

Do NOT stage or commit `.planning/`, `.gsd/`, `UI_PLAN.md`, this PLAN.md, or any SUMMARY.md —
the orchestrator owns docs commits. Stage `src/summary.rs` by explicit path; never `git add -A`.
Do NOT push.
  </action>
  <verify>
    <automated>rtk proxy cargo test --locked</automated>
    <automated>rtk proxy cargo test --locked --no-default-features</automated>
    <automated>rtk proxy cargo fmt --all --check</automated>
    <automated>rtk proxy cargo clippy --all-targets -- -D warnings</automated>
    <automated>rtk proxy git show --stat --format=%s HEAD</automated>
    <automated>rtk proxy git status --porcelain</automated>
  </verify>
  <done>
    All seven gate commands exited 0 with raw output read and recorded per check (a small
    table in the SUMMARY: check, exit code, observed result line). `git show --stat HEAD`
    shows the Part A subject line and `src/summary.rs` as the only file. `git status
    --porcelain` shows no tracked modifications and only the four pre-existing untracked
    paths plus this task's planning directory. Nothing pushed.
  </done>
</task>

<task type="auto">
  <name>Task 3: Bump to 0.12.1 and promote the CHANGELOG release section</name>
  <files>Cargo.toml, Cargo.lock, CHANGELOG.md</files>
  <read_first>
    `CHANGELOG.md:1-35` (the `## [Unreleased]` / `### Fixed` block and the `## [0.12.0] -
    2026-09-05` heading) and `CHANGELOG.md:837-839` (the link-reference block). The 0.12.0
    release commit `dc7e352` is the exact precedent for this edit — `git show dc7e352 --
    CHANGELOG.md Cargo.toml` shows the whole convention in one diff.
  </read_first>
  <precondition>Task 2's gate was green and Part A is committed — Part B starts only after the fix is proven.</precondition>
  <action>
Patch release: a bug fix only, no API, key-name or D-Bus interface change, so 0.12.0 → 0.12.1.

1. `Cargo.toml`: `version = "0.12.0"` → `version = "0.12.1"` (line 3). Change nothing else.

2. Refresh `Cargo.lock` so the `usbeehive` package entry reads 0.12.1 — otherwise EVERY
   `--locked` command fails with "the lock file needs to be updated". Run
   `rtk proxy cargo update -p usbeehive --offline`. If that errors, fall back to
   `rtk proxy cargo check` WITHOUT `--locked`, which regenerates the entry. Confirm with
   `rtk proxy git diff --stat Cargo.lock` that exactly one line changed (precedent: `dc7e352`
   touched Cargo.lock 1 line). If more than the version line moved, stop and report — a
   dependency re-resolution is out of scope for a patch release.

3. `CHANGELOG.md`, following `dc7e352` EXACTLY:
   - Keep the `## [Unreleased]` heading and its blank line in place, EMPTY.
   - Insert `## [0.12.1] - 2026-09-17` immediately below it, above the existing `### Fixed`.
   - Fold THIS task's fix into that `### Fixed` section alongside the previous task's entry:
     a second bullet (matching the existing bold-lead-sentence prose style) saying the hint
     now keys on the current the ACTIVE PDO offers rather than the maximum the charger
     advertises anywhere, so a device on a 9V/3A contract from a charger that also advertises
     20V/5A no longer trips it; and that the property is now condition-identical to the
     `CableNoEMarker` diagnostic on both axes. State the user-visible narrowing explicitly:
     a port where the kernel marks no PDO active no longer raises the hint (matching the
     diagnostic's long-standing behaviour). Say plainly this narrows an existing key's firing
     condition and is not an interface change — the key name is unchanged and the interface
     stays `org.usbeehive.Devices5`.
   - Link references at the foot of the file: change the `[Unreleased]` compare range to
     `v0.12.1...HEAD`, and add `[0.12.1]: https://github.com/abrauchli/usbeehive/compare/v0.12.0...v0.12.1`
     directly above the `[0.12.0]` line.

4. Commit Part B as ONE atomic commit over exactly `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`
   (same three-file shape as `dc7e352`), using the repo's release-commit subject style:

     `Release 0.12.1 — cable.no_emarker false-positive fixes`

   Body: both fixes in one patch release — the requested-current guard (260917-mdl) and the
   active-PDO charger-side basis (this task) — with the property and the diagnostic now
   pinned on both axes. No interface change. End with the
   `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>` line.

**HARD STOP for this task:** do NOT `git push` anything, do NOT `cargo publish`, and do NOT
create the `v0.12.1` tag. The tag is ORCHESTRATOR-OWNED — it creates the annotated tag after
you return, so it is guaranteed to land on the final commit. Creating it here would race that.
  </action>
  <verify>
    <automated>rtk proxy sh -c "grep -qx 'version = \"0.12.1\"' Cargo.toml"</automated>
    <automated>rtk proxy cargo build --locked</automated>
    <automated>rtk proxy sh -c "grep -qx '## \[0.12.1\] - 2026-09-17' CHANGELOG.md"</automated>
    <automated>rtk proxy sh -c "grep -q '^\[0.12.1\]: https://github.com/abrauchli/usbeehive/compare/v0.12.0\.\.\.v0.12.1$' CHANGELOG.md"</automated>
    <automated>rtk proxy sh -c "grep -q '^\[Unreleased\]: https://github.com/abrauchli/usbeehive/compare/v0.12.1\.\.\.HEAD$' CHANGELOG.md"</automated>
    <automated>rtk proxy git show --stat --format=%s HEAD</automated>
    <automated>rtk proxy sh -c 'test -z "$(git tag --list v0.12.1)"'</automated>
  </verify>
  <done>
    `Cargo.toml` reads 0.12.1 and `cargo build --locked` exits 0, which PROVES the lock is in
    sync (this is the gate, not a grep). The CHANGELOG has the dated 0.12.1 heading, an empty
    `## [Unreleased]` above it, both fix bullets under `### Fixed`, and both link references
    in the `dc7e352` shape. `git show --stat HEAD` shows the release subject and exactly
    `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`. No `v0.12.1` tag exists. Nothing pushed,
    nothing published.
  </done>
</task>

<task type="auto">
  <name>Task 4: Post-bump gate re-run, packaging dry run, release-readiness report</name>
  <files>(verification only — no files modified)</files>
  <precondition>A cargo registry cache or network is reachable for `cargo publish --dry-run`; if neither is, the packaging check degrades to `cargo package` and that is reported, not treated as a release blocker.</precondition>
  <action>
1. Re-run the gate on the bumped tree so the release commit itself is proven green, not just
   the fix commit: `rtk proxy cargo build --locked`, `rtk proxy cargo test --locked`,
   `rtk proxy cargo build --locked --no-default-features`,
   `rtk proxy cargo test --locked --no-default-features`,
   `rtk proxy cargo fmt --all --check`,
   `rtk proxy cargo clippy --all-targets -- -D warnings`,
   `RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps`. MSRV 1.85 still SKIPPED.

2. Packaging dry run. Try STRICT first: `rtk proxy cargo publish --dry-run --locked`.
   The working tree carries untracked paths that PREDATE this task (`.gsd/`,
   `.planning/config.json`, `.planning/quick/20260509-rename-to-usbeehive/`, `UI_PLAN.md`)
   plus this task's own planning directory, and `.planning/` is not gitignored — so cargo may
   refuse on a dirty/untracked working directory. If and ONLY IF the failure is that
   dirty/untracked check, re-run as `rtk proxy cargo publish --dry-run --locked --allow-dirty`
   and report BOTH outcomes. `--allow-dirty` is safe here: a dry run uploads nothing.
   Distinguish clearly in the report between (a) a dirty-tree refusal, (b) a registry/network
   failure — if the index is unreachable, fall back to `rtk proxy cargo package --locked
   --allow-dirty` as a local packaging check and say so — and (c) a REAL packaging defect
   (bad metadata, missing README, a path dependency), which is a genuine release blocker and
   must be reported as one.

3. Credentials check — **existence only**. Run exactly
   `rtk proxy sh -c "test -f ~/.cargo/credentials.toml && echo present || echo absent"`.
   NEVER `cat`, `head`, `grep`, `echo` or otherwise read, print or quote the file's contents.
   Report only the single word.

4. Write the release-readiness report into the SUMMARY: per-check gate table (check, exit
   code, observed raw result line), the two commit hashes with subjects, packaging dry-run
   outcome, credentials present/absent, and the explicit statement of what remains
   OPERATOR-OWNED and was deliberately NOT done: `git push`, `git push --tags`,
   `cargo publish`, and (orchestrator-owned) creation of the annotated `v0.12.1` tag.

**HARD STOP, absolute:** no `git push` in any form, no `cargo publish` without `--dry-run`,
no tag creation. Pushing a tag fires the public GitHub Release workflow — that is the
operator's trigger, not ours.
  </action>
  <verify>
    <automated>rtk proxy cargo test --locked</automated>
    <automated>rtk proxy cargo test --locked --no-default-features</automated>
    <automated>rtk proxy cargo clippy --all-targets -- -D warnings</automated>
    <automated>rtk proxy sh -c "test -f ~/.cargo/credentials.toml && echo present || echo absent"</automated>
    <automated>rtk proxy git log --oneline -3</automated>
    <automated>rtk proxy git status --porcelain --branch</automated>
  </verify>
  <done>
    Every gate command exited 0 on the bumped tree. The packaging dry run has a reported
    outcome classified as dirty-tree / network / real-defect, with the `--allow-dirty` rerun
    result when applicable. Credentials reported as exactly `present` or `absent`, contents
    never read. `git log --oneline -3` shows the two new commits on top of `ba93907`. The
    branch line shows no `ahead`-then-pushed state — nothing was pushed, published or tagged.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| kernel sysfs → summary | Untrusted/absent `current_now`, `voltage_now` and PDO fields cross into the property logic |
| local repo → crates.io / GitHub | An outward-facing, irreversible publish or tag push crosses here |
| local repo → registry credentials | `~/.cargo/credentials.toml` is a secret that must never enter the transcript |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-260917-mpq-01 | Information disclosure | `~/.cargo/credentials.toml` | critical | mitigate | Task 4 step 3 runs a `test -f` existence probe only; `cat`/`head`/`grep`/`echo` of the file are forbidden outright |
| T-260917-mpq-02 | Elevation of privilege | `git push`, `cargo publish`, `git tag` | critical | mitigate | Explicit HARD STOP in Tasks 3 and 4; dry run only, and only with `--dry-run`; the `v0.12.1` tag is orchestrator-owned so nothing here can race it onto the wrong commit |
| T-260917-mpq-03 | Tampering (of the verification signal) | `rtk` output filtering | high | mitigate | Every cargo command runs as `rtk proxy` and is judged by exit code on raw output; grep-for-`test result:` gates are banned by the `<critical_tooling_caveat>` block |
| T-260917-mpq-04 | Denial of service (panic on cast) | `src/summary.rs:1127` `requested_ma` | medium | mitigate | Carried over from 260917-mdl: the `.filter(|&ua| ua > 0)` must survive, asserted by a Task 1 verify gate. The new active current comes from `PowerDataObject.current_ma`, already `u32` (`src/power.rs:50`) — no new cast is introduced |
| T-260917-mpq-05 | Tampering (supply chain) | dependency graph | low | accept | No package is installed and no dependency is added. `cargo update -p usbeehive --offline` re-resolves only the root package's own version; Task 3 asserts Cargo.lock moved by exactly one line, which would catch an unintended re-resolution. No Package Legitimacy Audit is required for this plan |
| T-260917-mpq-06 | Repudiation | `org.usbeehive.Devices5` consumers | medium | accept | The firing condition narrows again, one release after the first narrowing. Accepted and made discoverable: the CHANGELOG 0.12.1 entry states the narrowing (including the no-active-PDO case) explicitly. Key name, `src/dbus.rs` and the interface name are untouched, so the `../usbee` consumer keeps matching |
</threat_model>

<inferred_decisions>
Record these in the SUMMARY under `## Inferred Decisions — Marked for Audit`, plus any new
ones taken during execution:

- **ID-01 (CLOSED by this task)** — the charger-side basis is unified with the diagnostic's
  active-PDO current. The divergence recorded as an open audit item in 260917-mdl is gone.
- **ID-01-a** — `cable_no_emarker_property_fires_for_big_charger_without_cable_node` gains
  `is_active: true` on its 20V/3.25A PDO. Report the empirical RED observation that forced it
  and argue why the test's stated intent is preserved, not weakened.
- **ID-01-b** — a port where the kernel marks no PDO `is_active` no longer raises the hint at
  all. This is the exact objection ID-01 raised against unification, now accepted: the
  diagnostic has always behaved this way, and condition-identity is the goal.
- **ID-01-c** — `cable_no_emarker_property_silent_for_3a_only_charger` gains `is_active: true`
  so the reason in its name (a charger that never exceeds 3A) is what keeps it silent, rather
  than the vacuous absence of an active PDO.
- **ID-03 (repeat)** — MSRV 1.85 build skipped; no 1.85 toolchain installed, installing one
  out of scope, and no new language or std feature is used.
- **ID-04 (repeat)** — proceeded without ROADMAP/STATE/PROJECT; none exist in this repo.
- **ID-05 (repeat)** — committed directly on `master`, per `git.branching_strategy: "none"`
  and the repo's master-direct history.
- **ID-06** — 0.12.1 chosen as a PATCH bump: behaviour narrows for an existing advisory key,
  but no API, key name, D-Bus signature or interface version changes.
</inferred_decisions>

<verification>
Phase-level checks, all judged by EXIT CODE on `rtk proxy` raw output:

1. `rtk proxy cargo test --locked` and `rtk proxy cargo test --locked --no-default-features`
   both exit 0 on the final commit.
2. `rtk proxy cargo fmt --all --check`, `rtk proxy cargo clippy --all-targets -- -D warnings`
   and `RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps` all exit 0, with no `allow`
   attribute added to achieve clippy silence.
3. Condition identity: `rtk proxy grep -n 'active_ma > 3_100' src/summary.rs src/diagnostic.rs`
   returns exactly two lines, one per file; the same for `2_900..=3_100`.
4. Scope: `rtk proxy git diff --stat ba93907..HEAD` touches exactly `src/summary.rs`,
   `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` — no `src/dbus.rs`, no `src/output.rs`, no
   `.planning/specs/`, nothing under `../usbee`.
5. Interface intact: `rtk proxy grep -rn 'cable.no_emarker' src/` shows the key string
   byte-identical, and `rtk proxy git diff --stat ba93907..HEAD -- src/dbus.rs` is empty.
6. Nothing outward-facing happened: no `v0.12.1` tag exists, no push, no publish.
</verification>

<success_criteria>
- A device on an active 9V/3A contract from a charger that also advertises 20V/5A, with no
  cable node, does NOT report `cable.no_emarker` — asserted by a named regression test.
- A device whose ACTIVE PDO exceeds 3.1A with a 3A-pinned request and no cable node still
  DOES report it — asserted by the existing `..._fires_...` test.
- `src/summary.rs` looks up the active PDO exactly once and derives both the contract power
  figure and the hint's charger-side current from it.
- The full CI gate is green for both feature sets on the final commit.
- Exactly two commits on top of `ba93907`: the fix (`src/summary.rs` only) and the release
  (`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` only).
- The crate is at 0.12.1 with a dated CHANGELOG section and correct link refs, locally
  committed and NOT pushed, NOT published, NOT tagged.
</success_criteria>

<output>
Create `.planning/quick/260917-mpq-close-id-01-cable-no-emarker-active-pdo-/260917-mpq-SUMMARY.md`
when done. Do NOT commit it or this PLAN.md — the orchestrator owns docs commits and creates
the annotated `v0.12.1` tag after you return.
</output>
