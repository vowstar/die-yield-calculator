# Gross Die calibration data contract

This contract is for offline validation of an anonymous Gross Die calibration
set. Do not commit confidential production data, product names, customer names,
or the private mapping from opaque identifiers to source records.

The validator uses an independent rectangular-grid geometry calculation, then
compares the unmodified geometry baseline with proportional and affine
corrections using leave-one-project-out cross-validation. It does not validate
die yield or Good Dies Per Wafer.

## CSV columns

The CSV header must contain exactly these columns:

| Column | Contract |
| --- | --- |
| `project_id` | Anonymous code matching `P` plus at least three digits. Reuse it for multiple observations from the same project. |
| `source_ref` | Opaque audit code matching `S` plus at least three digits. It must identify exactly one project, and every observation from that project must reuse the same code. Keep its private provenance mapping outside the public repository. |
| `length_unit` | Literal `mm`. |
| `scribe_unit` | Literal `um`. Values are converted to millimetres by dividing by 1,000. |
| `wafer_diameter_mm` | Nominal diameter, not radius. Valid range: 25–450 mm. |
| `edge_exclusion_mm` | Radial exclusion measured inward from the nominal edge. |
| `die_width_mm` | Post-shrink finished-die width, including seal ring and excluding scribe; 0.25 mm through the wafer diameter. |
| `die_height_mm` | Post-shrink finished-die height, including seal ring and excluding scribe; 0.25 mm through the wafer diameter. |
| `die_area_mm2` | Redundant unit check. It must agree with width × height within 0.1%. |
| `scribe_x_um` | Separation between adjacent die columns. |
| `scribe_y_um` | Separation between adjacent die rows. |
| `offset_x_mm` | Die-center grid phase relative to the wafer center. Whole-pitch-equivalent values are normalized. |
| `offset_y_mm` | Die-center grid phase relative to the wafer center. Whole-pitch-equivalent values are normalized. |
| `edge_policy` | `finished_die` when the finished die must fit, or `pitch_cell` when its complete pitch cell must fit. |
| `dimension_basis` | Literal `finished_die_including_seal_ring`. Pre-shrink photo dimensions are not accepted. |
| `wafer_shape` | Literal `circular_notch_ignored`. A notch or flat is not part of this geometry oracle. |
| `target_definition` | Literal `complete_die_sites`. Do not mix expected good die, partial die, or yield-adjusted counts into this target. |
| `reported_gross_die` | Audited discrete positive integer count under the declared counting convention. Estimates, ranges, uncertain measurements, and yield-adjusted values are not accepted. |

The redundant unit fields and area value are intentional. They make a silent
millimetre/micrometre conversion or pre-shrink/post-shrink mix easier to reject.
The validator also rejects a reported count above the usable-wafer physical
area bound. Such a failure commonly indicates a diameter/radius or area-unit
mistake and must be resolved before fitting.

`reported_gross_die` is an audited enumeration, not a continuous measurement
or statistical die-yield estimate. The combination of `target_definition`,
`edge_policy`, `dimension_basis`, `wafer_shape`, and the documented offsets is
the required counting convention. Reject a record when its source does not
establish those choices unambiguously; do not coerce an estimate, interval, or
rounded yield-adjusted value into an integer target.

The project and source codes form a one-to-one provenance group. Repeated
observations from one project reuse the same pair. A `source_ref` cannot appear
under another project ID, so one audited source cannot be relabeled to inflate
the declared project/source-group gate.

## Run the validator

```sh
python3 scripts/validate_gross_die_calibration.py /path/to/calibration.csv
```

Keep confidential production records outside the repository. The repository
also ignores `/private/` as a last-resort safeguard, but an external controlled
location remains preferable.

Machine-readable output is available with `--json`. The default coefficient
release gates are:

- at least 12 declared one-to-one project/source groups;
- leave-one-project-out MAPE no greater than 1%;
- maximum absolute percentage error no greater than 2%;
- MAE improvement over the uncalibrated geometry baseline of at least 20%.

Each threshold has a command-line option. Any project with multiple records is
held out as one group, so observations from the same project cannot leak into
its validation fold. The one-to-one source rule is validated before fitting;
cross-validation remains grouped by project ID. Opaque identifiers can enforce
the declared grouping but cannot prove that the underlying sources are
independent. Confirm that property against the private provenance record.

Fits and reported errors are record-weighted. Repeated records from one project
are held out together but still contribute one observation each. Prefer one
audited record per project unless repeated observations are substantively
justified, and review project record counts before releasing a coefficient.

The report includes MAE, median absolute error, MAPE, maximum absolute
percentage error, and MAE improvement for identity, proportional, and affine
models. The simplest fitted model that passes every gate is selected. Final
full-data coefficients are printed only after those gates pass. If the geometry
baseline already meets the absolute error gates but no fitted model passes the
improvement gate, the tool recommends the baseline without publishing
coefficients. Otherwise it exits with status 3 and reports no recommendation.

Exit status 2 means the CSV or gate arguments are invalid. A validation failure
must be corrected at the source; do not relax release gates merely to force a
coefficient recommendation.

## Required review before publication

- Confirm every diameter against the source; never infer it from an inch label.
- Confirm shrink has already been applied exactly once.
- Confirm seal-ring and scribe dimensions are not included twice.
- Confirm the edge and offset policies describe the reported count.
- Confirm the reported target is an audited discrete count under the declared
  convention, not an estimate, interval, uncertain measurement, or yield result.
- Inspect residuals by project and geometry range, even after numeric gates pass.
- Record the validator revision, gate values, anonymous dataset hash, supported
  input ranges, and validation metrics with any released calibration version.
- Treat inputs outside the validated ranges as out of domain.

Passing this tool is evidence that a small, anonymous dataset satisfies the
defined numerical gates. It is not evidence of fab-wide accuracy or real-user
validation.
