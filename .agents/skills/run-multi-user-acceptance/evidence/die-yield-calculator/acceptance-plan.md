# Multi-user acceptance plan

This plan uses simulated cognitive walkthroughs to find usability and
correctness risks. Simulated satisfaction is a test proxy, not evidence from
real customers.

## Release gates

- At least 80% of simulated personas are satisfied.
- At least 80% of persona-task cells pass.
- All novice and domain-expert core tasks pass.
- All required numerical correctness cases pass.
- No blocker or major finding remains open.

Correctness is a veto gate and cannot be averaged into a UX score.

## Baseline findings and final outcome

Round 0 found a polished visual system and a sound exact-grid geometry plus
Murphy-triangular yield core, but it failed the combined gate. Gross Dies was not
a primary result, statistical and geometric losses were visually mixed,
advanced controls dominated the novice path, narrow screens put the map before
the inputs, input precision was too low for the proposed calibration records,
and model assumptions were not directly auditable.

The proposed calibration examples are quarantined until their definitions are
corrected. A 150 mm diameter wafer cannot contain the reported die counts by
area alone, the displayed one-decimal yields do not distinguish Poisson from
Murphy triangular, and the displayed linear coefficients do not reproduce the
listed integer predictions under ordinary rounding.

Independent reconstruction strongly supports one physically consistent
interpretation while preserving the provenance limit. Under the supplied
classic formula and reconstructed die-footprint areas, the two classic counts
imply usable radii of approximately 147 mm. This is consistent with a 300 mm
nominal diameter, a 150 mm nominal radius, and approximately 3 mm radial edge
exclusion. The `150 mm (6 in)` entry is not a valid diameter for those counts:
either a radius was mislabeled as wafer size or a 300 mm selection was
transcribed incorrectly. Existing evidence cannot distinguish those clerical
failure paths or prove the original production convention.

After the implementation and regression rounds, the v0.2.0 release artifact
passes the structured gate: 4/4 simulated personas satisfied, 19/20 persona
tasks passed, 13/13 required correctness cases passed, and no blocker or major
finding remains open
([round-7-v0.2.0-release.json](rounds/round-7-v0.2.0-release.json)).
Scenario comparison remains a moderate, non-core expert efficiency gap. These
scores are simulated acceptance, not measured customer satisfaction.

Rounds 8–10 audited a second semantic risk: D₀ could be interpreted as a raw
baseline or per-mask value even though the built-in area models require an
effective full-process random-fatal-defect density. The v0.2.1 release makes that
contract visible in the UI and reproducible in SVG and JSON without adding a
process-complexity coefficient or changing any calculation. The final focused
gate passes with 4/4 simulated personas, 8/8 persona tasks, 8/8 correctness cases,
and no open blocker or major finding
([round-10-v0.2.1-release.json](rounds/round-10-v0.2.1-release.json)).

Rounds 11–13 audit constrained-width and input-safety risks. The tagged v0.2.1
baseline exposed clipped phone controls, inefficient tablet breakpoints, small
touch targets, background focus behind Report, and touch gestures that could
change a number while the user intended to scroll. The focused implementation
uses responsive row stacking, 44-point interactions, click-only touch fields
with an explicit editor, a true modal report workflow, and finite field-aware
parsers. Standard browser accessibility APIs still cannot enumerate every egui
canvas control; the labeled canvas, complete keyboard path, and optional
app-spoken feedback are documented alternatives, not a claim of equivalent
screen-reader support. Round 12 passes with 4/5 simulated personas satisfied,
22/23 persona tasks passed, and every required correctness case passed
([round-12-responsive-focused.json](rounds/round-12-responsive-focused.json)).

Round 13 freezes the v0.2.2 artifact and repeats the numerical, report-parity,
and release gates on the hash-frozen trunk build. It passes with 4/5 simulated
personas satisfied, 22/23 persona tasks passed, and 8/8 required correctness
cases passed. The native release report and the browser export are byte-identical
(sha256 fe01f45174017be68988fb986fe99f1076534a880653601f5fb2b9e8aff9e4cc), and
`trunk build --release` reproduces the frozen WASM byte-for-byte. No blocker or
major finding remains open
([round-13-responsive-final.json](rounds/round-13-responsive-final.json)).

## Implementation slices

### 1. Truthful primary outputs

- Promote Gross Dies / Wafer to the first result.
- Keep geometric, random-defect, boundary, and probe quantities separate.
- Expose active yield area, placement pitch, defect exposure, selected model,
  unrounded expectation, and display-rounding policy.
- Preserve enough input and report precision for sub-micrometre scribe data.
- Add Poisson, Murphy triangular, Seeds, and Negative Binomial behind one model
  API with independent formula and limit tests.

### 2. Progressive interaction

- Present essential inputs first and reveal manufacturing, model, alignment,
  and probe controls progressively.
- Put the result summary first, inputs second, and the large map last on narrow
  screens.
- Keep the last valid result visible but clearly paused during invalid edits.
- Show one error summary plus field-local correction text.
- Remove silent changes to still-valid user inputs.

### 3. Calibration readiness

- Define an anonymous, unit-explicit calibration data contract.
- Validate physical area bounds and target definitions before fitting.
- Compare an exact-geometry baseline with simple residual corrections using
  leave-one-project-out validation.
- Publish no coefficient unless it improves out-of-sample error and passes the
  configured MAE/MAPE/max-error gates.
- Display model version, sample count, supported range, and out-of-domain state
  when a future validated calibration is integrated.

### 4. Placement and expert workflow

- Explore placement phase before attributing its effect to an empirical fit.
- Add phase optimization only with a bounded algorithm and an honest optimality
  claim.
- Keep reproducible JSON interchange as the audit foundation; add retained
  side-by-side scenario comparison in a future expert-workflow slice.

### 5. Responsive and input-safe interaction

- Use one-column phone and tablet-portrait layouts and complete two-column
  layouts when the page has at least 960 logical pixels of content width.
- Stack labels above controls only when the local card width requires it; do not
  infer touch capability from a narrow column inside an otherwise desktop page.
- Give primary controls and disclosure rows a 44-point interaction height.
- On constrained or touch-capable viewports, separate a tap-to-edit numeric
  action from vertical scroll gestures and validate the value before applying it.
- Reject NaN, infinity, non-numeric text, and fractional integer fields without
  changing the last valid model.
- Keep Report focus and scrolling inside a real modal, then restore focus to the
  invoking control after close.

## Recorded test rounds

1. **Round 0 — baseline:** novice, expert, skeptical verifier, and constrained
   viewport/keyboard walkthroughs plus independent numerical oracles.
2. **Round 1 — focused rerun:** repeat the same tasks after slices 1 and 2; do
   not remove failing cases.
3. **Round 2 — adversarial regression:** attack units, precision, invalid and
   extreme inputs, hidden state, model limits, narrow layouts, exports, and
   calibration-domain boundaries.
4. **Rounds 3–4 — evidence hardening:** repeat keyboard recovery, exact report
   semantics, and native/browser parity; retain failures when an assigned path
   lacks direct evidence.
5. **Round 5 — final-evidence challenge:** preserve the 13/13 numerical pass but
   fail release after a fresh expert path exposes high-precision input
   truncation.
6. **Round 6 — pre-version release candidate:** independently repeat novice, expert,
   verifier, and constrained-keyboard tasks on a hash-frozen WASM artifact and
   require every release gate to pass.
7. **Round 7 — v0.2.0 release:** rebuild the versioned artifact, repeat the
   persona, numerical, calibration, export-parity, keyboard, and screenshot
   challenges, and preserve the zero-participant limitation in the evidence.
8. **Rounds 8–10 — D₀ semantics and v0.2.1 release:** fail the tagged baseline
   on input-basis ambiguity, rerun focused novice/expert/export/narrow tasks after
   the clarification, then repeat release, report-parity, and screenshot gates on
   the versioned artifact.
9. **Rounds 11–13 — responsive and touch-safe v0.2.2 release:** fail the tagged
   baseline on phone, tablet, touch, modal, and accessibility-entry tasks; rerun
   the focused matrix after implementation; then freeze the versioned artifact
   and repeat numerical, report, screenshot, and release gates.

Each round is evaluated with
`.agents/skills/run-multi-user-acceptance/scripts/evaluate_acceptance.py`.

## Deferred inputs

Real calibration coefficients remain deferred until anonymized source records
resolve shrink semantics, seal-ring inclusion, scribe units, edge policy, phase
policy, target definition, and integer rounding. The supplied example's physical
wafer interpretation is treated as most consistent with a 300 mm nominal
diameter, but the source of
its incorrect `150 mm (6 in)` label remains unknown. Critical-area analysis
remains deferred until layout-derived critical area and defect-size data exist.
A separate process-complexity model remains deferred until users can supply an
authoritative factor definition; the existing negative-binomial alpha continues
to mean defect clustering only.
