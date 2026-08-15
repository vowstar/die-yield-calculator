# Die-yield correctness checks

## Contents

1. Quantity boundaries
2. Geometry oracle
3. Random-defect yield oracles
4. Expected-good-die semantics
5. Calibration checks
6. Adversarial cases

## 1. Quantity boundaries

Keep these quantities explicit and do not use them as synonyms:

- active die dimensions and area;
- scribe-lane dimensions;
- placement pitch;
- nominal wafer diameter;
- usable radius after edge exclusion;
- complete geometric dies;
- partial and excluded sites;
- statistical die yield;
- expected good dies.

Record whether a source dimension is pre-shrink or post-shrink and whether seal ring or scribe is already included. Never apply a factor twice. Convert `mm²` to `cm²` by dividing by 100.

## 2. Geometry oracle

For a rectangular grid, independently calculate:

```text
pitch_x = active_width + column_scribe
pitch_y = active_height + row_scribe
usable_radius = wafer_diameter / 2 - edge_exclusion
center_x = origin_x + column_index * pitch_x
center_y = origin_y + row_index * pitch_y
```

Count a candidate only when the chosen inclusion footprint satisfies the documented edge policy. If the policy requires the active rectangle to fit, all four active-die corners must lie within the usable circle. If it requires the pitch or seal-ring footprint, use that footprint instead. Treat the policy choice as an input or documented assumption, not an implementation detail.

Use a small, independent enumerator or hand-countable fixtures as the oracle. Do not call the production grid builder. Check orientation, half-pitch origin, offsets modulo pitch, asymmetric scribes, and candidates tangent to the usable boundary.

Do not assume every discrete die count is smoothly monotonic when phase or optimization policy changes. Explain any count discontinuity by inspecting the placement grid.

## 3. Random-defect yield oracles

Let:

```text
A = active or critical area in cm², explicitly labeled
lambda = A * D0
```

Verify these models independently:

```text
Poisson:             exp(-lambda)
Murphy triangular:   ((1 - exp(-lambda)) / lambda)^2
Seeds:               1 / (1 + lambda)
Negative binomial:   (1 + lambda / alpha)^(-alpha), alpha > 0
```

Use the analytic limit `yield = 1` when `lambda = 0`. Negative binomial must equal Seeds at `alpha = 1` and approach Poisson as `alpha` grows. Yield must remain in `[0, 1]` and decrease as nonnegative area or defect density increases.

Verify the software's selected model label matches the implemented formula. Do not compare a Murphy result to a Poisson reference while calling it an error.

## 4. Expected-good-die semantics

Keep the unrounded expectation available:

```text
expected_good_exact = geometric_usable * yield
```

Document the display rounding policy. A colored wafer map derived from the expectation is illustrative; it does not predict the physical locations of future defective dies. Label deterministic pseudo-random markers as modeled or illustrative loss.

Keep geometric loss separate from random-defect loss. A partial die is not a statistically defective complete die.

## 5. Calibration checks

Prefer a physical geometry baseline plus a fitted residual correction over an unconstrained black-box prediction. Require each calibration record to identify:

- anonymous project ID and source;
- wafer and usable-area policy;
- active dimensions, shrink status, seal ring, and scribes;
- grid orientation and offset policy when known;
- reported target definition;
- measurement uncertainty or counting convention.

With a small dataset, compare the uncalibrated baseline, proportional correction, linear correction, and effective-edge parameter. Use leave-one-project-out validation, report MAE/MAPE/max error, and retain the simplest model that improves out-of-sample performance. Show sample count, model version, validation error, supported input range, and out-of-domain warning in the product.

Do not ship coefficients inferred from two examples or from unavailable data. Do not expose confidential product names in UI, fixtures, screenshots, reports, or public documentation.

## 6. Adversarial cases

Include:

- zero defect density and near-zero exposure;
- very high density without NaN or negative yield;
- zero and asymmetric scribe lanes;
- edge exclusion approaching the wafer radius;
- minimum and maximum supported die sizes;
- whole-pitch-equivalent offsets;
- half-pitch origin changes;
- active-versus-pitch edge inclusion;
- unit paste and conversion errors;
- invalid, non-finite, and out-of-range values;
- stored settings from an older model version;
- report/export values matching the live summary;
- narrow viewport and keyboard-only completion;
- calibration inputs outside the validated range.
