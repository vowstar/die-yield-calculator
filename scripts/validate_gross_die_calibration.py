#!/usr/bin/env python3
"""Validate and cross-validate anonymous Gross Die calibration data.

The tool deliberately uses only the Python standard library. It builds an
independent rectangular-grid geometry baseline, validates physical and unit
invariants, and evaluates only simple calibration models with grouped
leave-one-project-out cross-validation.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import statistics
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable, Iterable, Sequence


REQUIRED_COLUMNS = (
    "project_id",
    "source_ref",
    "length_unit",
    "scribe_unit",
    "wafer_diameter_mm",
    "edge_exclusion_mm",
    "die_width_mm",
    "die_height_mm",
    "die_area_mm2",
    "scribe_x_um",
    "scribe_y_um",
    "offset_x_mm",
    "offset_y_mm",
    "edge_policy",
    "dimension_basis",
    "wafer_shape",
    "target_definition",
    "reported_gross_die",
)

PROJECT_ID_PATTERN = re.compile(r"P[0-9]{3,}")
SOURCE_REF_PATTERN = re.compile(r"S[0-9]{3,}")
MAX_CANDIDATES = 2_000_000
AREA_RELATIVE_TOLERANCE = 1.0e-3
AREA_ABSOLUTE_TOLERANCE_MM2 = 1.0e-6
DEFAULT_MINIMUM_PROJECTS = 12


class DatasetError(ValueError):
    """One or more CSV records violate the calibration data contract."""

    def __init__(self, errors: Sequence[str]) -> None:
        self.errors = tuple(errors)
        super().__init__("; ".join(self.errors))


@dataclass(frozen=True)
class CalibrationRow:
    """One validated calibration observation."""

    row_number: int
    project_id: str
    source_ref: str
    wafer_diameter_mm: float
    edge_exclusion_mm: float
    die_width_mm: float
    die_height_mm: float
    die_area_mm2: float
    scribe_x_um: float
    scribe_y_um: float
    offset_x_mm: float
    offset_y_mm: float
    edge_policy: str
    reported_gross_die: int
    geometric_baseline: int
    physical_upper_bound: int


@dataclass(frozen=True)
class Gates:
    """Explicit release gates for fitted coefficients."""

    minimum_projects: int = DEFAULT_MINIMUM_PROJECTS
    maximum_mape_percent: float = 1.0
    maximum_ape_percent: float = 2.0
    minimum_mae_improvement_percent: float = 20.0


@dataclass(frozen=True)
class Metrics:
    """Cross-validation errors for one model."""

    mae_die: float
    median_ae_die: float
    mape_percent: float
    max_ape_percent: float
    mae_improvement_percent: float


@dataclass(frozen=True)
class ModelResult:
    """Availability, metrics, and gate result for one candidate model."""

    model: str
    complexity_rank: int
    metrics: Metrics | None
    passes_coefficient_gates: bool
    reason: str | None


@dataclass(frozen=True)
class Decision:
    """Calibration recommendation after all gates are evaluated."""

    status: str
    model: str | None
    message: str
    coefficients: dict[str, float] | None


@dataclass(frozen=True)
class Evaluation:
    """Complete machine-readable evaluation result."""

    record_count: int
    project_count: int
    gates: Gates
    models: tuple[ModelResult, ...]
    decision: Decision
    validated_domain: dict[str, list[float]]


@dataclass(frozen=True)
class Point:
    project_id: str
    baseline: float
    actual: float


def _parse_finite(row: dict[str, str], field: str, row_number: int) -> float:
    raw = row[field].strip()
    try:
        value = float(raw)
    except ValueError as error:
        raise DatasetError([f"row {row_number}: {field} must be numeric"]) from error
    if not math.isfinite(value):
        raise DatasetError([f"row {row_number}: {field} must be finite"])
    return value


def _parse_positive_integer(row: dict[str, str], field: str, row_number: int) -> int:
    raw = row[field].strip()
    if not re.fullmatch(r"[0-9]+", raw):
        raise DatasetError(
            [
                f"row {row_number}: {field} must be an audited discrete positive "
                "integer count"
            ]
        )
    value = int(raw)
    if value <= 0:
        raise DatasetError(
            [
                f"row {row_number}: {field} must be an audited discrete positive "
                "integer count"
            ]
        )
    return value


def _canonical_phase(offset: float, pitch: float) -> float:
    wrapped = (offset + pitch / 2.0) % pitch - pitch / 2.0
    return 0.0 if abs(wrapped) <= pitch * 1.0e-12 else wrapped


def geometric_gross_die(
    *,
    wafer_diameter_mm: float,
    edge_exclusion_mm: float,
    die_width_mm: float,
    die_height_mm: float,
    scribe_x_um: float,
    scribe_y_um: float,
    offset_x_mm: float,
    offset_y_mm: float,
    edge_policy: str,
) -> int:
    """Count full sites with an independent axis-aligned geometry oracle."""

    pitch_x = die_width_mm + scribe_x_um / 1_000.0
    pitch_y = die_height_mm + scribe_y_um / 1_000.0
    usable_radius = wafer_diameter_mm / 2.0 - edge_exclusion_mm
    origin_x = _canonical_phase(offset_x_mm, pitch_x)
    origin_y = _canonical_phase(offset_y_mm, pitch_y)

    if edge_policy == "finished_die":
        half_width = die_width_mm / 2.0
        half_height = die_height_mm / 2.0
    elif edge_policy == "pitch_cell":
        half_width = pitch_x / 2.0
        half_height = pitch_y / 2.0
    else:
        raise ValueError(f"unsupported edge policy: {edge_policy}")

    first_column = math.floor((-usable_radius - half_width - origin_x) / pitch_x) - 1
    last_column = math.ceil((usable_radius + half_width - origin_x) / pitch_x) + 1
    first_row = math.floor((-usable_radius - half_height - origin_y) / pitch_y) - 1
    last_row = math.ceil((usable_radius + half_height - origin_y) / pitch_y) + 1
    candidate_count = (last_column - first_column + 1) * (last_row - first_row + 1)
    if candidate_count > MAX_CANDIDATES:
        raise DatasetError(
            [f"geometry produces {candidate_count:,} candidates; limit is {MAX_CANDIDATES:,}"]
        )

    radius_squared = usable_radius * usable_radius
    tolerance = max(radius_squared, 1.0) * 1.0e-12
    count = 0
    for row_index in range(first_row, last_row + 1):
        center_y = origin_y + row_index * pitch_y
        farthest_y = abs(center_y) + half_height
        for column_index in range(first_column, last_column + 1):
            center_x = origin_x + column_index * pitch_x
            farthest_x = abs(center_x) + half_width
            if farthest_x * farthest_x + farthest_y * farthest_y <= radius_squared + tolerance:
                count += 1
    return count


def _validate_and_build(row: dict[str, str], row_number: int) -> CalibrationRow:
    errors: list[str] = []
    project_id = row["project_id"].strip()
    source_ref = row["source_ref"].strip()
    if PROJECT_ID_PATTERN.fullmatch(project_id) is None:
        errors.append(f"row {row_number}: project_id must use an anonymous code such as P001")
    if SOURCE_REF_PATTERN.fullmatch(source_ref) is None:
        errors.append(f"row {row_number}: source_ref must use an opaque code such as S001")

    fixed_values = {
        "length_unit": "mm",
        "scribe_unit": "um",
        "dimension_basis": "finished_die_including_seal_ring",
        "wafer_shape": "circular_notch_ignored",
        "target_definition": "complete_die_sites",
    }
    for field, expected in fixed_values.items():
        actual = row[field].strip()
        if actual != expected:
            errors.append(
                f"row {row_number}: {field} must be {expected!r}, got {actual!r}"
            )

    edge_policy = row["edge_policy"].strip()
    if edge_policy not in {"finished_die", "pitch_cell"}:
        errors.append(
            f"row {row_number}: edge_policy must be 'finished_die' or 'pitch_cell'"
        )

    parsed: dict[str, float] = {}
    for field in (
        "wafer_diameter_mm",
        "edge_exclusion_mm",
        "die_width_mm",
        "die_height_mm",
        "die_area_mm2",
        "scribe_x_um",
        "scribe_y_um",
        "offset_x_mm",
        "offset_y_mm",
    ):
        try:
            parsed[field] = _parse_finite(row, field, row_number)
        except DatasetError as error:
            errors.extend(error.errors)

    try:
        reported_gross_die = _parse_positive_integer(row, "reported_gross_die", row_number)
    except DatasetError as error:
        errors.extend(error.errors)
        reported_gross_die = 0

    if errors:
        raise DatasetError(errors)

    diameter = parsed["wafer_diameter_mm"]
    exclusion = parsed["edge_exclusion_mm"]
    width = parsed["die_width_mm"]
    height = parsed["die_height_mm"]
    stated_area = parsed["die_area_mm2"]
    scribe_x = parsed["scribe_x_um"]
    scribe_y = parsed["scribe_y_um"]

    if not 25.0 <= diameter <= 450.0:
        errors.append(f"row {row_number}: wafer_diameter_mm must be between 25 and 450")
    if exclusion < 0.0 or exclusion >= diameter / 2.0:
        errors.append(
            f"row {row_number}: edge_exclusion_mm must be non-negative and smaller than the radius"
        )
    if not 0.25 <= width <= diameter or not 0.25 <= height <= diameter:
        errors.append(
            f"row {row_number}: die dimensions must be between 0.25 mm and the wafer diameter"
        )
    if scribe_x < 0.0 or scribe_y < 0.0:
        errors.append(f"row {row_number}: scribe dimensions must be non-negative")
    if scribe_x > 10_000.0 or scribe_y > 10_000.0:
        errors.append(
            f"row {row_number}: scribe dimensions exceed 10,000 um; check the declared unit"
        )

    calculated_area = width * height
    area_tolerance = max(
        AREA_ABSOLUTE_TOLERANCE_MM2,
        calculated_area * AREA_RELATIVE_TOLERANCE,
    )
    if stated_area <= 0.0 or abs(stated_area - calculated_area) > area_tolerance:
        errors.append(
            f"row {row_number}: die_area_mm2={stated_area:g} is inconsistent with "
            f"die_width_mm * die_height_mm={calculated_area:g}"
        )

    if errors:
        raise DatasetError(errors)

    usable_radius = diameter / 2.0 - exclusion
    pitch_x = width + scribe_x / 1_000.0
    pitch_y = height + scribe_y / 1_000.0
    footprint_area = width * height
    if edge_policy == "pitch_cell":
        footprint_area = pitch_x * pitch_y
    physical_upper_bound = math.floor(
        math.pi * usable_radius * usable_radius / footprint_area + 1.0e-12
    )
    if reported_gross_die > physical_upper_bound:
        errors.append(
            f"row {row_number}: reported_gross_die={reported_gross_die} exceeds the physical "
            f"area bound {physical_upper_bound}; check wafer diameter/radius and length units"
        )

    try:
        baseline = geometric_gross_die(
            wafer_diameter_mm=diameter,
            edge_exclusion_mm=exclusion,
            die_width_mm=width,
            die_height_mm=height,
            scribe_x_um=scribe_x,
            scribe_y_um=scribe_y,
            offset_x_mm=parsed["offset_x_mm"],
            offset_y_mm=parsed["offset_y_mm"],
            edge_policy=edge_policy,
        )
    except DatasetError as error:
        errors.extend(f"row {row_number}: {message}" for message in error.errors)
        baseline = 0

    if baseline <= 0:
        errors.append(
            f"row {row_number}: documented geometry produces no complete die sites"
        )
    if errors:
        raise DatasetError(errors)

    return CalibrationRow(
        row_number=row_number,
        project_id=project_id,
        source_ref=source_ref,
        wafer_diameter_mm=diameter,
        edge_exclusion_mm=exclusion,
        die_width_mm=width,
        die_height_mm=height,
        die_area_mm2=stated_area,
        scribe_x_um=scribe_x,
        scribe_y_um=scribe_y,
        offset_x_mm=parsed["offset_x_mm"],
        offset_y_mm=parsed["offset_y_mm"],
        edge_policy=edge_policy,
        reported_gross_die=reported_gross_die,
        geometric_baseline=baseline,
        physical_upper_bound=physical_upper_bound,
    )


def load_csv(path: Path) -> list[CalibrationRow]:
    """Load a CSV file and return rows only when the entire dataset is valid."""

    errors: list[str] = []
    rows: list[CalibrationRow] = []
    try:
        input_file = path.open("r", encoding="utf-8-sig", newline="")
    except OSError as error:
        raise DatasetError([f"unable to read {path}: {error}"]) from error

    with input_file:
        reader = csv.DictReader(input_file)
        if reader.fieldnames is None:
            raise DatasetError(["CSV header is missing"])
        actual_columns = tuple(reader.fieldnames)
        duplicate_columns = sorted(
            {column for column in actual_columns if actual_columns.count(column) > 1}
        )
        missing = sorted(set(REQUIRED_COLUMNS) - set(actual_columns))
        unexpected = sorted(set(actual_columns) - set(REQUIRED_COLUMNS))
        if duplicate_columns:
            errors.append(f"duplicate columns: {', '.join(duplicate_columns)}")
        if missing:
            errors.append(f"missing columns: {', '.join(missing)}")
        if unexpected:
            errors.append(f"unexpected columns: {', '.join(unexpected)}")
        if errors:
            raise DatasetError(errors)

        for row_number, raw_row in enumerate(reader, start=2):
            if None in raw_row:
                errors.append(f"row {row_number}: too many CSV fields")
                continue
            if all((value or "").strip() == "" for value in raw_row.values()):
                continue
            normalized = {key: value or "" for key, value in raw_row.items()}
            try:
                rows.append(_validate_and_build(normalized, row_number))
            except DatasetError as error:
                errors.extend(error.errors)

    if not rows and not errors:
        errors.append("CSV contains no calibration records")

    sources_by_project: dict[str, set[str]] = {}
    projects_by_source: dict[str, set[str]] = {}
    for row in rows:
        sources_by_project.setdefault(row.project_id, set()).add(row.source_ref)
        projects_by_source.setdefault(row.source_ref, set()).add(row.project_id)
    for project_id, sources in sorted(sources_by_project.items()):
        if len(sources) != 1:
            errors.append(
                f"project {project_id} maps to multiple source_ref values: {', '.join(sorted(sources))}"
            )
    for source_ref, project_ids in sorted(projects_by_source.items()):
        if len(project_ids) != 1:
            errors.append(
                f"source {source_ref} maps to multiple project_id values: "
                f"{', '.join(sorted(project_ids))}"
            )

    if errors:
        raise DatasetError(errors)
    return rows


def _fit_identity(_: Sequence[Point]) -> tuple[float, float]:
    return 1.0, 0.0


def _fit_proportional(points: Sequence[Point]) -> tuple[float, float]:
    denominator = sum(point.baseline * point.baseline for point in points)
    if denominator <= 0.0:
        raise ValueError("proportional fit has no positive baseline values")
    scale = sum(point.baseline * point.actual for point in points) / denominator
    if not math.isfinite(scale) or scale <= 0.0:
        raise ValueError("proportional fit produced a non-positive scale")
    return scale, 0.0


def _fit_affine(points: Sequence[Point]) -> tuple[float, float]:
    if len(points) < 2:
        raise ValueError("affine fit requires at least two training records")
    mean_x = statistics.fmean(point.baseline for point in points)
    mean_y = statistics.fmean(point.actual for point in points)
    denominator = sum((point.baseline - mean_x) ** 2 for point in points)
    if denominator <= 1.0e-12:
        raise ValueError("affine fit requires distinct geometric baseline values")
    slope = sum(
        (point.baseline - mean_x) * (point.actual - mean_y) for point in points
    ) / denominator
    intercept = mean_y - slope * mean_x
    if not math.isfinite(slope) or not math.isfinite(intercept) or slope <= 0.0:
        raise ValueError("affine fit produced a non-monotonic correction")
    minimum_x = min(point.baseline for point in points)
    maximum_x = max(point.baseline for point in points)
    if slope * minimum_x + intercept <= 0.0 or slope * maximum_x + intercept <= 0.0:
        raise ValueError("affine fit predicts a non-positive count in the observed domain")
    return slope, intercept


MODEL_FITTERS: dict[str, tuple[int, Callable[[Sequence[Point]], tuple[float, float]]]] = {
    "identity": (0, _fit_identity),
    "proportional": (1, _fit_proportional),
    "affine": (2, _fit_affine),
}


def _cross_validated_predictions(points: Sequence[Point], model: str) -> list[tuple[float, float]]:
    if model == "identity":
        return [(point.actual, point.baseline) for point in points]

    fitter = MODEL_FITTERS[model][1]
    projects = sorted({point.project_id for point in points})
    predictions: list[tuple[float, float]] = []
    for held_out_project in projects:
        training = [point for point in points if point.project_id != held_out_project]
        held_out = [point for point in points if point.project_id == held_out_project]
        slope, intercept = fitter(training)
        for point in held_out:
            prediction = slope * point.baseline + intercept
            if not math.isfinite(prediction) or prediction <= 0.0:
                raise ValueError(
                    f"{model} predicts a non-positive count for held-out project {held_out_project}"
                )
            predictions.append((point.actual, prediction))
    return predictions


def _metrics(
    actual_and_predicted: Iterable[tuple[float, float]],
    identity_mae: float | None = None,
) -> Metrics:
    pairs = list(actual_and_predicted)
    absolute_errors = [abs(actual - predicted) for actual, predicted in pairs]
    percentage_errors = [
        abs(actual - predicted) / actual * 100.0 for actual, predicted in pairs
    ]
    mae = statistics.fmean(absolute_errors)
    if identity_mae is None or identity_mae <= 1.0e-12:
        improvement = 0.0
    else:
        improvement = (identity_mae - mae) / identity_mae * 100.0
    return Metrics(
        mae_die=mae,
        median_ae_die=statistics.median(absolute_errors),
        mape_percent=statistics.fmean(percentage_errors),
        max_ape_percent=max(percentage_errors),
        mae_improvement_percent=improvement,
    )


def _validated_domain(rows: Sequence[CalibrationRow]) -> dict[str, list[float]]:
    fields = {
        "wafer_diameter_mm": [row.wafer_diameter_mm for row in rows],
        "die_area_mm2": [row.die_area_mm2 for row in rows],
        "scribe_x_um": [row.scribe_x_um for row in rows],
        "scribe_y_um": [row.scribe_y_um for row in rows],
        "geometric_baseline": [float(row.geometric_baseline) for row in rows],
    }
    return {name: [min(values), max(values)] for name, values in fields.items()}


def evaluate(rows: Sequence[CalibrationRow], gates: Gates) -> Evaluation:
    """Evaluate candidate models and return a gated recommendation."""

    points = [
        Point(row.project_id, float(row.geometric_baseline), float(row.reported_gross_die))
        for row in rows
    ]
    project_count = len({point.project_id for point in points})
    identity_metrics = _metrics(_cross_validated_predictions(points, "identity"))
    model_results: list[ModelResult] = [
        ModelResult(
            model="identity",
            complexity_rank=0,
            metrics=identity_metrics,
            passes_coefficient_gates=False,
            reason="identity has no fitted coefficients",
        )
    ]

    eligible_fitted_models: list[str] = []
    for model in ("proportional", "affine"):
        complexity_rank = MODEL_FITTERS[model][0]
        try:
            predictions = _cross_validated_predictions(points, model)
            metrics = _metrics(predictions, identity_metrics.mae_die)
            failures: list[str] = []
            if project_count < gates.minimum_projects:
                failures.append(
                    "requires at least "
                    f"{gates.minimum_projects} declared project/source groups"
                )
            if metrics.mape_percent > gates.maximum_mape_percent:
                failures.append(
                    f"MAPE exceeds {gates.maximum_mape_percent:g}%"
                )
            if metrics.max_ape_percent > gates.maximum_ape_percent:
                failures.append(
                    f"max APE exceeds {gates.maximum_ape_percent:g}%"
                )
            if metrics.mae_improvement_percent < gates.minimum_mae_improvement_percent:
                failures.append(
                    "MAE improvement is below "
                    f"{gates.minimum_mae_improvement_percent:g}%"
                )
            passes = not failures
            if passes:
                eligible_fitted_models.append(model)
            model_results.append(
                ModelResult(
                    model=model,
                    complexity_rank=complexity_rank,
                    metrics=metrics,
                    passes_coefficient_gates=passes,
                    reason="; ".join(failures) if failures else None,
                )
            )
        except ValueError as error:
            model_results.append(
                ModelResult(
                    model=model,
                    complexity_rank=complexity_rank,
                    metrics=None,
                    passes_coefficient_gates=False,
                    reason=str(error),
                )
            )

    if eligible_fitted_models:
        selected_model = min(
            eligible_fitted_models,
            key=lambda model: MODEL_FITTERS[model][0],
        )
        slope, intercept = MODEL_FITTERS[selected_model][1](points)
        decision = Decision(
            status="calibrated_model_recommended",
            model=selected_model,
            message=(
                f"{selected_model} is the simplest fitted model that passes every explicit "
                "leave-one-project-out gate"
            ),
            coefficients={"slope": slope, "intercept_die": intercept},
        )
    elif (
        identity_metrics.mape_percent <= gates.maximum_mape_percent
        and identity_metrics.max_ape_percent <= gates.maximum_ape_percent
    ):
        decision = Decision(
            status="geometry_baseline_sufficient",
            model="identity",
            message=(
                "No fitted model passed every coefficient gate; the uncalibrated geometry "
                "baseline already passes the absolute error gates"
            ),
            coefficients=None,
        )
    else:
        decision = Decision(
            status="no_recommendation",
            model=None,
            message=(
                "No fitted model passed every coefficient gate and the geometry baseline "
                "does not meet the absolute error gates"
            ),
            coefficients=None,
        )

    return Evaluation(
        record_count=len(rows),
        project_count=project_count,
        gates=gates,
        models=tuple(model_results),
        decision=decision,
        validated_domain=_validated_domain(rows),
    )


def _format_optional(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.4f}"


def render_text(evaluation: Evaluation) -> str:
    """Render a concise human-readable validation report."""

    lines = [
        "VALID DATASET",
        f"records: {evaluation.record_count}",
        f"declared project/source groups: {evaluation.project_count}",
        (
            "gates: "
            f"projects>={evaluation.gates.minimum_projects}, "
            f"MAPE<={evaluation.gates.maximum_mape_percent:g}%, "
            f"max APE<={evaluation.gates.maximum_ape_percent:g}%, "
            "MAE improvement>="
            f"{evaluation.gates.minimum_mae_improvement_percent:g}%"
        ),
        "",
        "MODEL          MAE(die)  MEDIAN_AE  MAPE(%)  MAX_APE(%)  IMPROVEMENT(%)  GATE",
    ]
    for result in evaluation.models:
        metrics = result.metrics
        if metrics is None:
            lines.append(
                f"{result.model:<14} {'n/a':>8}  {'n/a':>9}  {'n/a':>7}  "
                f"{'n/a':>10}  {'n/a':>14}  FAIL"
            )
        else:
            gate = "PASS" if result.passes_coefficient_gates else "FAIL"
            if result.model == "identity":
                gate = "BASE"
            lines.append(
                f"{result.model:<14} {metrics.mae_die:>8.4f}  "
                f"{metrics.median_ae_die:>9.4f}  {metrics.mape_percent:>7.4f}  "
                f"{metrics.max_ape_percent:>10.4f}  "
                f"{metrics.mae_improvement_percent:>14.4f}  {gate}"
            )
        if result.reason and result.model != "identity":
            lines.append(f"  reason: {result.reason}")

    lines.extend(
        [
            "",
            f"DECISION: {evaluation.decision.status}",
            evaluation.decision.message,
        ]
    )
    if evaluation.decision.coefficients is not None:
        coefficients = evaluation.decision.coefficients
        lines.append(
            "recommended equation: reported_gross_die = "
            f"{_format_optional(coefficients.get('slope'))} * geometric_baseline + "
            f"{_format_optional(coefficients.get('intercept_die'))}"
        )
    else:
        lines.append("recommended coefficients: none")
    return "\n".join(lines)


def _evaluation_as_dict(evaluation: Evaluation) -> dict[str, object]:
    return asdict(evaluation)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Validate anonymous Gross Die calibration data and gate simple models with "
            "leave-one-project-out cross-validation."
        )
    )
    parser.add_argument("csv_path", type=Path)
    parser.add_argument("--json", action="store_true", help="write machine-readable JSON")
    parser.add_argument(
        "--min-projects",
        type=int,
        default=DEFAULT_MINIMUM_PROJECTS,
        help=(
            "minimum declared project/source groups required before fitted "
            f"coefficients can be recommended (default: {DEFAULT_MINIMUM_PROJECTS})"
        ),
    )
    parser.add_argument("--max-mape-percent", type=float, default=1.0)
    parser.add_argument("--max-ape-percent", type=float, default=2.0)
    parser.add_argument("--min-mae-improvement-percent", type=float, default=20.0)
    return parser


def _validated_gates(arguments: argparse.Namespace) -> Gates:
    errors: list[str] = []
    if arguments.min_projects < 3:
        errors.append("--min-projects must be at least 3")
    for name in (
        "max_mape_percent",
        "max_ape_percent",
        "min_mae_improvement_percent",
    ):
        value = getattr(arguments, name)
        if not math.isfinite(value) or value < 0.0:
            errors.append(f"--{name.replace('_', '-')} must be finite and non-negative")
    if errors:
        raise DatasetError(errors)
    return Gates(
        minimum_projects=arguments.min_projects,
        maximum_mape_percent=arguments.max_mape_percent,
        maximum_ape_percent=arguments.max_ape_percent,
        minimum_mae_improvement_percent=arguments.min_mae_improvement_percent,
    )


def main(argv: Sequence[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        gates = _validated_gates(arguments)
        rows = load_csv(arguments.csv_path)
        evaluation = evaluate(rows, gates)
    except DatasetError as error:
        if arguments.json:
            print(json.dumps({"status": "invalid_dataset", "errors": error.errors}, indent=2))
        else:
            print("INVALID DATASET", file=sys.stderr)
            for message in error.errors:
                print(f"- {message}", file=sys.stderr)
        return 2

    if arguments.json:
        print(json.dumps(_evaluation_as_dict(evaluation), indent=2, sort_keys=True))
    else:
        print(render_text(evaluation))
    return 3 if evaluation.decision.status == "no_recommendation" else 0


if __name__ == "__main__":
    raise SystemExit(main())
