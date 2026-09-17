---
phase: quick-260917-mdl
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/summary.rs
  - CHANGELOG.md
autonomous: true
requirements: [QUICK-260917-mdl]

estimate:
  tokens: 45000
  raw_tokens: 30000
  tasks: 3
  confidence: low   # no calibration samples for this repo

must_haves:
  truths:
    - "A port with a live 20V/5A (100W) contract and no cable node does NOT report `cable.no_emarker`."
    - "A port with a >3A charger, no cable node, and an RDO operating current pinned at ~3A DOES still report `cable.no_emarker`."
    - "A port with no power-supply node at all (requested current unknown) does NOT report `cable.no_emarker` — the hint now requires positive 3A-pin evidence."
    - "The `cable.no_emarker` property key string is unchanged, so `org.usbeehive.Devices5` stays intact and the ../usbee consumer keeps matching."
    - "The full local CI gate (build, test, fmt, clippy, doc — default and --no-default-features) is green, judged by exit code on raw (un-filtered) cargo output."
  artifacts:
    - "src/summary.rs — guard added at the `cable.no_emarker` push site; regression test added; existing `cable_no_emarker_*` tests reconciled."
    - "CHANGELOG.md — `## [Unreleased]` / `### Fixed` entry describing the false positive."
  key_links:
    - "src/summary.rs `requested_ma` (computed ~L1123-1129) must feed the new guard at the property push site (~L1163-1169)."
    - "The summary guard's numeric range must be byte-identical to `src/diagnostic.rs:103` `(2_900..=3_100).contains(&requested_ma)` so the property and the `Bottleneck::CableNoEMarker` verdict cannot disagree on the requested-current axis."
    - "`requested_ma`'s existing `.filter(|&ua| ua > 0)` must survive — it is what keeps a negative sysfs `current_now` out of the `as u32` cast."
---

<objective>
Fix the false-positive `cable.no_emarker` summary property reported upstream: on a UCSI
laptop with a live 20V @ 5A / 100W contract the property fires anyway, because the kernel
never registers a `/sys/class/typec/portN-cable/` node (the PPM does not answer
GET_CABLE_PROPERTY) and the property only ever consulted the *advertised* charger current,
never the *negotiated* one. A 5A contract is only legal over a 5A e-marked cable, so the
property contradicts the very payload it ships in.

Purpose: bring the summary property in line with the already-correct diagnostic at
`src/diagnostic.rs:101-104`, which requires the request to be pinned at ~3A before it calls
a cable non-e-marked. That consistency IS the fix.

Output: guarded property + regression test in `src/summary.rs`, a CHANGELOG entry, and a
green local CI gate.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@src/summary.rs
@src/diagnostic.rs
@CHANGELOG.md

No `.planning/PROJECT.md`, `.planning/ROADMAP.md` or `.planning/STATE.md` exist in this repo
and none are needed — do not attempt to read them. No project `CLAUDE.md` and no
`.claude/skills/` or `.agents/skills/` directory exist.
</context>

<critical_tooling_caveat>
**This environment rewrites bare bash commands through `rtk`, which STRIPS lines matching
`warning:` and `test result:` from cargo output.** A grep for either string therefore
succeeds vacuously and produces a false green. EVERY cargo invocation in this plan MUST be
written as `rtk proxy cargo ...` so the raw output is visible, and MUST be judged by exit
code plus the real output text. Do not substitute a bare `cargo ...` anywhere, including in
ad-hoc checks you invent while executing.
</critical_tooling_caveat>

<inferred_decisions>
The human operator is unavailable. These were decided from the brief and the code; each is
marked for later audit.

- **ID-01 — Charger-side condition left as-is (`max_advertised_ma > 3_000`).** The brief
  scopes the fix to adding a *requested/negotiated current* guard. Switching the
  charger-side basis to the diagnostic's `active_ma > 3_100` would change behavior beyond
  the reported bug (ports where no PDO is marked `is_active` would silently lose the hint
  entirely). **Residual divergence accepted and recorded:** when the active PDO is ≤3.1A but
  some *other* advertised PDO exceeds 3A, the property can still fire while the diagnostic
  does not emit `Bottleneck::CableNoEMarker`. Audit item: decide later whether to unify the
  charger-side basis too.
- **ID-02 — The existing test `cable_no_emarker_property_fires_for_big_charger_without_cable_node`
  (`src/summary.rs:2304`) legitimately needs a fixture change.** Its `TypeCPort` is built with
  `..Default::default()`, so `power_supply` is `None` and `requested_ma` is `0` — under the
  new guard it would stop firing. The guard is correct (no PSY = no 3A-pin evidence), so the
  fixture gains a live 3A-pinned contract. The test's intent (big charger + no cable node ⇒
  hint fires) is preserved, not weakened.
- **ID-03 — MSRV 1.85 build skipped.** `rustup toolchain list` shows stable, nightly, 1.87,
  1.88 — no 1.85. The brief forbids installing one. The change introduces no new language or
  std features (a `RangeInclusive::contains` call, stable since 1.35), so MSRV risk is nil.
- **ID-04 — Proceeding without ROADMAP/STATE/PROJECT**, per the dispatch note; this repo has
  run prior quick tasks without them.
</inferred_decisions>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: Guard `cable.no_emarker` on the negotiated operating current</name>
  <files>src/summary.rs</files>
  <read_first>
    - `src/diagnostic.rs:101-118` — the reference implementation. The conjunct to mirror is
      `(2_900..=3_100).contains(&requested_ma)` on line 103. Copy those numeric bounds
      verbatim; do NOT invent different thresholds.
    - `src/summary.rs:1119-1129` — `requested_ma` is already computed in scope (RDO operating
      current in mA, `0` when unknown), including a `.filter(|&ua| ua > 0)` that keeps a
      negative sysfs `current_now` out of the `as u32` cast. Leave that filter intact.
    - `src/summary.rs:1152-1169` — the property push site to modify.
    - `src/summary.rs:1877-1895` — `live_ucsi_power_reported_without_linked_pd_port`, the
      idiomatic `TypeCPowerSupply` fixture shape in this test module (`online`,
      `voltage_now_uv`, `current_now_ua`, `..Default::default()`).
    - `src/summary.rs:2303-2384` — the three existing `cable_no_emarker_*` tests.
    - `src/summary.rs:1966` — precedent for naming `crate::diagnostic::Bottleneck` inside the
      test module without a new import.
  </read_first>
  <behavior>
    - Regression (the reported bug): partner present; source caps advertise one 20V/5A/100W
      PDO with `is_active: true`; `power_supply` online with `voltage_now_uv: Some(20_000_000)`
      and `current_now_ua: Some(5_000_000)`; `cable` is `None`. ⇒ `cable.no_emarker` is
      ABSENT from `s.properties`, and `s.charging_diag` is not
      `crate::diagnostic::Bottleneck::CableNoEMarker`.
    - Still fires (the true positive): a 65W charger (20V/3.25A) with no cable node and an
      RDO operating current pinned at 3A ⇒ `cable.no_emarker == "true"` is PRESENT.
    - Unchanged: a 3A-only (15W) charger stays silent; a charger with a 5A e-marker
      `current_rating` present stays silent.
  </behavior>
  <action>
    Write the failing regression test FIRST, run it, and confirm it fails for the right
    reason (property present when it should be absent) before touching the production code.

    1. Add a test to the `mod tests` block in `src/summary.rs`, placed immediately after
    `cable_no_emarker_property_silent_when_emarker_rating_present` so the four
    `cable_no_emarker_*` cases stay contiguous. Name it
    `cable_no_emarker_property_absent_when_5a_contract_negotiated`. Build the fixture per
    `<behavior>` above, following the local convention of `use crate::power::{PdoType,
    PowerDataObject};` inside the test body and `crate::typec::TypeCPartner::default()` for
    the partner. Give it a comment explaining the upstream report: a UCSI PPM that never
    answers GET_CABLE_PROPERTY leaves `current_rating` permanently `None`, so the old
    condition could never be satisfied away by any cable — while the 5A contract it ships
    alongside is itself proof of a 5A e-marked cable. Assert both the absent property and
    the non-`CableNoEMarker` verdict, so the property and the bottleneck are pinned to agree.

    2. Make it pass: at the `cable.no_emarker` push site (`src/summary.rs:1163-1169`) add a
    fourth conjunct to the `if`, `(2_900..=3_100).contains(&requested_ma)`, using exactly the
    bounds from `src/diagnostic.rs:103`. Keep `port.partner.is_some()`,
    `max_advertised_ma > 3_000` and the `current_rating().is_none()` conjunct as they are
    (see ID-01). Extend the existing explanatory comment above the `if` to record that the
    hint now additionally requires the RDO operating current to sit at the spec's 3A default
    — an unknown (`0`) or higher request is not evidence of a non-e-marked cable — and that
    the bounds are held identical to `ChargingDiagnostic::evaluate` on purpose so the
    property and the bottleneck verdict cannot disagree.

    3. Reconcile the three pre-existing tests per ID-02. Run them and report the actual
    outcome; do not pre-emptively edit one that still passes.
    `cable_no_emarker_property_fires_for_big_charger_without_cable_node` (:2304) is expected
    to go red, because its port has no `power_supply` and therefore
    `requested_ma == 0`. Give it a `power_supply: Some(TypeCPowerSupply { online: true,
    voltage_now_uv: Some(20_000_000), current_now_ua: Some(3_000_000), ..Default::default() })`
    — a real 3A-pinned contract on the 65W charger — and extend its comment to say the 3A pin
    is now the evidence the hint keys on. `TypeCPowerSupply` is already imported at the top of
    the file (`src/summary.rs:26`), so `super::*` covers it. The other two
    (`..._silent_for_3a_only_charger` at :2333 and `..._silent_when_emarker_rating_present`
    at :2358) are expected to keep passing untouched, because each already fails an earlier
    conjunct. If either one in fact goes red, state which and why in the SUMMARY rather than
    adjusting it silently.

    Do NOT rename the `cable.no_emarker` key, do NOT touch `src/dbus.rs`, `.planning/specs/`
    or anything under `../usbee`, and do NOT bump the package version.
  </action>
  <verify>
    <automated>rtk proxy cargo test --locked --lib</automated>
    <automated>rtk proxy cargo test --locked --lib -- --list | grep -c 'cable_no_emarker'   # expect >= 4 (3 pre-existing + 1 new)</automated>
  </verify>
  <done>
    All four `cable_no_emarker_*` tests pass; the whole lib test suite passes with exit code
    0; the raw `test result: ok.` line was observed in un-filtered `rtk proxy` output (not
    inferred from a grep); the `cable.no_emarker` key string is byte-identical to before; the
    summary guard's numeric bounds match `src/diagnostic.rs:103` exactly.
  </done>
</task>

<task type="auto">
  <name>Task 2: CHANGELOG entry under Unreleased/Fixed</name>
  <files>CHANGELOG.md</files>
  <read_first>
    - `CHANGELOG.md:1-60` — Keep a Changelog format; `## [Unreleased]` currently sits at line
      8 with no subsections. The house style for entries is a bolded lead sentence followed
      by explanatory prose paragraphs, wrapped at roughly 72 columns.
  </read_first>
  <action>
    Add a `### Fixed` subsection under the existing `## [Unreleased]` heading and write one
    entry, matching the file's existing voice and wrap width. Lead with a bolded summary such
    as "**`cable.no_emarker` no longer fires against a live 5A contract.**" Then explain: the
    summary property checked only whether the charger advertised more than 3A somewhere in
    its capabilities and whether a cable e-marker current rating was visible — never the
    negotiated operating current. On UCSI systems whose PPM does not answer
    GET_CABLE_PROPERTY the kernel registers no cable node at all, so the rating is
    permanently absent and the hint fired on every big charger, including ports running a
    20V/5A 100W contract that is itself only legal over a 5A e-marked cable. The property now
    also requires the RDO operating current to sit at the spec's 3A default, matching the
    bounds `ChargingDiagnostic` has always used, so the property and the `CableNoEMarker`
    bottleneck agree.

    Note that this is a behavior change to an existing key's firing condition, not an
    interface change: the key name is unchanged and the D-Bus interface stays
    `org.usbeehive.Devices5`. Do not add an `### Added` or `### Changed` section, do not
    create a new version heading, and do not bump the version in `Cargo.toml`.
  </action>
  <verify>
    <automated>rtk proxy grep -n -A4 '^### Fixed' CHANGELOG.md</automated>
  </verify>
  <done>
    `## [Unreleased]` contains a `### Fixed` subsection with one entry describing the false
    positive and the new requested-current condition; no version heading was added; no other
    CHANGELOG section was modified.
  </done>
</task>

<task type="auto">
  <name>Task 3: Run the full local CI gate green</name>
  <files>(no files modified — verification only; fix-ups land in src/summary.rs if needed)</files>
  <read_first>
    - `.github/workflows/ci.yml` — four jobs: `test` (matrix over `""` and
      `--no-default-features`), `lint` (fmt + clippy), `msrv` (1.85 build), `docs` (rustdoc
      with `-D warnings`).
  </read_first>
  <action>
    Reproduce every CI job locally. Run each command through `rtk proxy` — see
    `<critical_tooling_caveat>`; a bare `cargo` here yields filtered output in which a
    `warning:` line that clippy or rustdoc emitted is invisible, and the gate reads green
    while CI goes red. Judge each command by its exit code AND by reading its raw output.

    Commands, in order:
    - `rtk proxy cargo build --locked`
    - `rtk proxy cargo test --locked`
    - `rtk proxy cargo build --locked --no-default-features`
    - `rtk proxy cargo test --locked --no-default-features`
    - `rtk proxy cargo fmt --all --check`
    - `rtk proxy cargo clippy --all-targets -- -D warnings`
    - `RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps`

    Skip the MSRV job (ID-03): `rustup toolchain list` shows stable, nightly, 1.87 and 1.88
    but no 1.85, and installing a toolchain is out of scope. If a 1.85 toolchain turns out to
    already be present, run `rtk proxy cargo +1.85 build --locked` as well.

    If any command fails, fix the cause in `src/summary.rs` (or `CHANGELOG.md`) and re-run
    the affected command plus everything downstream of it. Do not silence a clippy lint with
    an `allow` attribute to get green — fix the code. In the SUMMARY, record for each command
    its exit code and the concrete result line you actually saw in the raw output (e.g. the
    `test result: ok. N passed; 0 failed` line for each test invocation, both feature sets).
  </action>
  <verify>
    <automated>rtk proxy cargo test --locked &amp;&amp; rtk proxy cargo test --locked --no-default-features &amp;&amp; rtk proxy cargo fmt --all --check &amp;&amp; rtk proxy cargo clippy --all-targets -- -D warnings</automated>
    <automated>RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps</automated>
  </verify>
  <done>
    All seven commands exited 0. Both `cargo test` invocations printed a real
    `test result: ok.` line in raw `rtk proxy` output with a non-zero passed count. clippy
    produced no warnings with `-D warnings` and no new `allow` attribute was introduced to
    achieve that. `cargo doc` produced no rustdoc warnings. The MSRV job is recorded as
    skipped with its reason.
  </done>
</task>

</tasks>

<threat_model>
Security enforcement is active (`security_enforcement` unset ⇒ enabled); ASVS level 1,
blocking threshold `high`.

## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| kernel sysfs → usbeehive | `/sys/class/typec/**` values (`current_now`, source capability PDOs, cable nodes) are attacker-influenceable: a malicious or merely buggy USB-C partner chooses what it advertises and what contract is negotiated. |
| usbeehive summary → consumers | `s.properties` is serialized to the CLI, JSON output, and the `org.usbeehive.Devices5` D-Bus `a(ss)` bag consumed by ../usbee. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-260917-mdl-01 | Spoofing | `src/summary.rs` `cable.no_emarker` push site | low | accept | A hostile partner can advertise caps/negotiate a current that suppresses or induces the hint. The property is an advisory display string; no authz, trust or safety decision keys off it. Accepting is what ASVS L1 calls for on a read-only informational field. |
| T-260917-mdl-02 | Denial of Service | `src/summary.rs:1123-1129` `requested_ma` | medium | mitigate | `current_now_ua` is `i64` and can be negative (`src/typec.rs:266` exercises `Some(-1)`). The existing `.filter(\|&ua\| ua > 0)` is the only thing keeping a negative out of the `(ua / 1000) as u32` cast, where it would wrap to a huge value. Task 1 explicitly preserves that filter; the new `(2_900..=3_100)` bound is applied to the already-filtered value. |
| T-260917-mdl-03 | Tampering | `org.usbeehive.Devices5` key surface | low | mitigate | Renaming or removing the `cable.no_emarker` key would break the exact-string consumer contract (`property-policy.js:130`, `label-table.js:52,173`) and require a `Devices6` bump per the rule at `src/dbus.rs:45`. The plan changes only the firing condition; the key string is unchanged and `src/dbus.rs` is untouched. |
| T-260917-mdl-04 | Information Disclosure | summary output | low | accept | No new data is read from sysfs and no new field is emitted; the change strictly narrows when an existing advisory string appears. |
| T-260917-mdl-SC | Tampering | dependency supply chain | low | accept | No package-manager install is in scope — no `Cargo.toml` dependency is added, removed or bumped, and every build/test command carries `--locked`, so a drifted `Cargo.lock` fails the gate rather than resolving silently. No package legitimacy checkpoint is required. |

No threat is rated `high` or above, so nothing here blocks the plan.
</threat_model>

<capability_checkpoints>
- **API coverage:** detector run over this task's scope returned `detected: false` (exit 1,
  no signals). No external API/SDK integration in scope — checkpoint skipped, no
  `COVERAGE.md` required.
- **Assumption-delta:** requires a resolvable ROADMAP phase section; this repo has no
  `ROADMAP.md`, so the probe is `skipped` (`phase_unresolved`), **not** `detected: false`.
  Checkpoint skipped for this run per the fragment's own skip semantics; advisory only, never
  blocking.
- **Schema push gate:** no Payload/Prisma/Drizzle/Supabase/TypeORM file patterns are in scope
  (this is a Rust crate; the files touched are `src/summary.rs` and `CHANGELOG.md`). Skipped
  silently; no `[BLOCKING]` push task injected.
- **Security:** `<threat_model>` above, ASVS L1 / block-on-high.
</capability_checkpoints>

<verification>
1. `rtk proxy cargo test --locked` and `rtk proxy cargo test --locked --no-default-features`
   both exit 0 with a real, observed `test result: ok.` line.
2. The new `cable_no_emarker_property_absent_when_5a_contract_negotiated` test exists and
   passes; it fails if the guard conjunct is reverted (confirmed by having seen it red first).
3. `rtk proxy git diff --stat` touches exactly `src/summary.rs` and `CHANGELOG.md` — no
   `src/dbus.rs`, no `Cargo.toml`, no `.planning/specs/`.
4. `rtk proxy grep -rn 'cable.no_emarker' src/` shows the key string unchanged.
5. fmt, clippy (`-D warnings`) and rustdoc (`RUSTDOCFLAGS="-D warnings"`) all exit 0 on raw
   output.
</verification>

<success_criteria>
- A port with a negotiated 20V/5A contract and no cable node no longer reports
  `cable.no_emarker`, and its diagnostic is not `CableNoEMarker`.
- A >3A charger with the request pinned at ~3A and no visible e-marker still reports it.
- The summary guard's requested-current bounds are byte-identical to `src/diagnostic.rs:103`.
- The `cable.no_emarker` key name, `src/dbus.rs`, the consumer specs and the package version
  are all untouched.
- `CHANGELOG.md` has an `## [Unreleased]` / `### Fixed` entry in the file's existing style.
- The full local CI gate is green, every cargo command run under `rtk proxy` and judged on
  exit code plus raw output.
- ID-01 through ID-04 are carried into the SUMMARY as audit items.
</success_criteria>

<output>
Create `.planning/quick/260917-mdl-fix-false-positive-cable-no-emarker-summ/260917-mdl-SUMMARY.md` when done.

The SUMMARY must state explicitly:
- Which pre-existing tests were touched and why (ID-02), or that none were.
- The observed `test result:` line for each of the two feature sets.
- That the MSRV 1.85 job was skipped and why (ID-03).
- The ID-01 residual divergence, restated as an open audit item.
</output>
