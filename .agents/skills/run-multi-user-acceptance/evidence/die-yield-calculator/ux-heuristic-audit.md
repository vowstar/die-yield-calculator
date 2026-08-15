# UX heuristic audit

## Scope and evidence

This audit evaluates two primary outcomes: a first-time user can obtain and
correctly interpret a result, and a domain expert can inspect assumptions,
precision, and reproducibility without cluttering the first-use path. A skeptical
verifier and a keyboard- or narrow-screen user were included as challenge roles.

The findings are based on the structured baseline walkthrough in
[round-0-baseline.json](rounds/round-0-baseline.json), seven subsequent recorded
acceptance rounds, current UI source, isolated scripted browser/native walkthroughs,
and automated tests. The final frozen-artifact evidence is in
[round-7-v0.2.0-release.json](rounds/round-7-v0.2.0-release.json).
Source-backed behavior is not presented as observed user sentiment. Numerical
correctness remains a separate veto gate; visual clarity cannot compensate for
an incorrect result.

## Implemented mitigations

### Progressive disclosure

The default setup keeps wafer diameter, active die dimensions, defect density,
and random-defect model visible. Manufacturing geometry, model details, grid
alignment, probe estimation, and geometry details are collapsed until requested.
Their headers summarize the current values or selected model, so experts retain
direct access without making beginners parse every control
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)).

This is intended to reduce initial decision load while preserving a predictable path from
essential inputs to increasingly specialized assumptions.

### Recognition instead of recall

Wafer presets show their diameter class, inputs include units, and the selected
yield model remains visible in both the selector and its disclosure header. When
opened, model details show the equation and interpretation rather than requiring
the user to remember model definitions
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)). Manufacturing headers also
surface edge exclusion and X/Y scribe values before expansion.

### Visible assumptions and calibrated trust

The setup identifies loaded defaults as examples and asks the user to verify
dimensions, edge policy, and defect density. Model details expose yield area in
mm² and cm², dimensionless exposure `A·D₀`, the unrounded expected-good value,
and the whole-die rounding policy
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)).

The map states that modeled loss locations are illustrative and that the notch
marker is not subtracted from geometry. The footer and exported report describe
the result as a planning estimate rather than production evidence
([app.rs](../../../../../crates/die-yield-gui/src/app.rs),
[report.rs](../../../../../crates/die-yield-gui/src/report.rs)). These
qualifications are intended to calibrate trust instead of implying certainty
from a polished display.

### Geometry and statistical loss remain distinct

The first result is `Gross Dies / Wafer`: complete geometric sites before modeled
defects. Statistical die yield and expected good dies are separate results, with
the exact multiplication shown under the rounded expectation
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)). The map legend independently
names gross die, illustrative loss, partial boundary, and edge band, and the
manufacturing note distinguishes placement pitch from active yield area
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)).

This separation limits a common category error: treating a geometrically lost or
partial site as a statistically defective complete die.

### Error recovery and last-valid state

Invalid edits retain the last valid analysis instead of replacing it with an
empty or impossible result. A prominent paused-state message identifies the
stale result, a summary lists every invalid field, local messages appear beside
the relevant controls, and report export is disabled until the setup is valid
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)). An automated regression test
checks preservation of the last valid analysis
([app.rs](../../../../../crates/die-yield-gui/src/app.rs)).

Focused numeric controls are restored across valid/invalid layout transitions,
while their edit buffers are synchronized only when validation state changes.
Isolated scripted Firefox/WASM regressions verified both sides of that tradeoff: an invalid
`40` mm edge exclusion can be replaced with `3` without reacquiring focus, and
character-by-character entry with a human-like inter-key delay retains
`10.123456` rather than truncating it. This supports
recovery without silently presenting stale values as current or damaging valid
high-precision entry.

### Precision and auditability

Finished-die, density, and phase controls retain up to six decimal places;
scribe lanes are entered explicitly in micrometres. Expert disclosures show
normalized phase, placement pitch, usable diameter, boundary counts, and exact
expectations. Reports carry the selected model and assumptions, while the report
layer can produce a deterministic JSON snapshot with normalized inputs, exact and
rounded results, rounding semantics, and map caveats
([app.rs](../../../../../crates/die-yield-gui/src/app.rs),
[report.rs](../../../../../crates/die-yield-gui/src/report.rs)).

This precision is for faithful entry and traceability; it is not a claim that the
underlying process data are known to the same number of digits.

### Narrow and keyboard ordering

Below the wide-layout threshold, the primary summary appears first, followed by
calculation setup and then the larger wafer map. This gives a first-time user an
immediate answer while keeping editable assumptions ahead of the exploratory
visualization ([app.rs](../../../../../crates/die-yield-gui/src/app.rs)). A compact scope
note beside the summary distinguishes complete gross sites, boundary exclusions,
and illustrative random loss without requiring the geometry disclosure.

Focused controls request scrolling into view, including controls inside
progressive disclosures. Page Up and Page Down provide viewport navigation,
top-bar actions have explicit focus rings, and the report dialog supports Escape
to close ([app.rs](../../../../../crates/die-yield-gui/src/app.rs)). Responsive source tests cover
360 through 1440 logical pixels. Firefox/WASM walkthroughs at 500 by 1000
exercised page navigation, visible focus, disclosures, report
open/close, preset selection, and no-Tab error recovery. Native and browser
wide/narrow captures were also inspected. Screen-reader behavior and
supported-platform coverage still require real assistive-technology testing;
keyboard simulation alone is not accessibility validation.

## Remaining non-major expert gaps

- **Scenario comparison:** there is no side-by-side baseline/candidate view,
  delta explanation, or named scenario library. The deterministic snapshot
  format is a useful foundation, but comparison remains a manual expert task.
- **Future validated calibration:** an anonymous, unit-explicit offline workflow
  exists, but no fitted coefficient is bundled or claimed. Any future calibrated
  result must expose its version, independent-project count, validation errors,
  supported range, and out-of-domain state before it can be treated as a trusted
  application result.

Neither gap makes the current geometric or statistical result incorrect; both
limit efficiency or confidence in more advanced workflows.

## Research caution

Persona satisfaction in this project is a simulated acceptance proxy derived
from task completion and explicit gates. It is not evidence that real users are
satisfied, understand the terminology, or prefer the interaction. Claims about
comprehension, workflow fit, and accessibility should be confirmed with real
participants and supported assistive technologies. Future acceptance rounds must
repeat the same core tasks and preserve correctness as a non-negotiable gate.
