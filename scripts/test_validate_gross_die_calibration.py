#!/usr/bin/env python3
"""Deterministic self-tests for validate_gross_die_calibration.py."""

from __future__ import annotations

import csv
import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

from validate_gross_die_calibration import (
    REQUIRED_COLUMNS,
    DatasetError,
    Gates,
    Point,
    _cross_validated_predictions,
    evaluate,
    geometric_gross_die,
    load_csv,
    main,
)


def _base_record(project_number: int, die_width: float, die_height: float) -> dict[str, str]:
    baseline = geometric_gross_die(
        wafer_diameter_mm=300.0,
        edge_exclusion_mm=3.0,
        die_width_mm=die_width,
        die_height_mm=die_height,
        scribe_x_um=80.0,
        scribe_y_um=100.0,
        offset_x_mm=0.0,
        offset_y_mm=0.0,
        edge_policy="finished_die",
    )
    return {
        "project_id": f"P{project_number:03d}",
        "source_ref": f"S{project_number:03d}",
        "length_unit": "mm",
        "scribe_unit": "um",
        "wafer_diameter_mm": "300",
        "edge_exclusion_mm": "3",
        "die_width_mm": f"{die_width:.6f}",
        "die_height_mm": f"{die_height:.6f}",
        "die_area_mm2": f"{die_width * die_height:.9f}",
        "scribe_x_um": "80",
        "scribe_y_um": "100",
        "offset_x_mm": "0",
        "offset_y_mm": "0",
        "edge_policy": "finished_die",
        "dimension_basis": "finished_die_including_seal_ring",
        "wafer_shape": "circular_notch_ignored",
        "target_definition": "complete_die_sites",
        "reported_gross_die": str(baseline),
    }


class TemporaryCsv:
    def __init__(self, records: list[dict[str, str]]) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "calibration.csv"
        with self.path.open("w", encoding="utf-8", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=REQUIRED_COLUMNS)
            writer.writeheader()
            writer.writerows(records)

    def close(self) -> None:
        self.directory.cleanup()


class CalibrationValidationTests(unittest.TestCase):
    def test_geometry_is_invariant_under_whole_pitch_offsets(self) -> None:
        arguments = {
            "wafer_diameter_mm": 300.0,
            "edge_exclusion_mm": 3.0,
            "die_width_mm": 8.0,
            "die_height_mm": 6.0,
            "scribe_x_um": 80.0,
            "scribe_y_um": 100.0,
            "edge_policy": "finished_die",
        }
        baseline = geometric_gross_die(offset_x_mm=0.0, offset_y_mm=0.0, **arguments)
        shifted = geometric_gross_die(
            offset_x_mm=8.08,
            offset_y_mm=-12.2,
            **arguments,
        )
        self.assertEqual(baseline, shifted)

    def test_rejects_diameter_radius_inconsistency_by_area_bound(self) -> None:
        record = _base_record(1, 4.0, 5.66)
        record["wafer_diameter_mm"] = "150"
        record["edge_exclusion_mm"] = "0"
        record["reported_gross_die"] = "2882"
        fixture = TemporaryCsv([record])
        self.addCleanup(fixture.close)

        with self.assertRaisesRegex(DatasetError, "physical area bound"):
            load_csv(fixture.path)

    def test_rejects_redundant_area_unit_mismatch(self) -> None:
        record = _base_record(1, 4.0, 5.56)
        record["die_area_mm2"] = "0.2224"
        fixture = TemporaryCsv([record])
        self.addCleanup(fixture.close)

        with self.assertRaisesRegex(DatasetError, "inconsistent"):
            load_csv(fixture.path)

    def test_rejects_wrong_declared_scribe_unit(self) -> None:
        record = _base_record(1, 4.0, 5.56)
        record["scribe_unit"] = "mm"
        fixture = TemporaryCsv([record])
        self.addCleanup(fixture.close)

        with self.assertRaisesRegex(DatasetError, "scribe_unit"):
            load_csv(fixture.path)

    def test_rejects_ambiguous_reported_count(self) -> None:
        record = _base_record(1, 4.0, 5.56)
        record["reported_gross_die"] = "2882 +/- 5"
        fixture = TemporaryCsv([record])
        self.addCleanup(fixture.close)

        with self.assertRaisesRegex(DatasetError, "audited discrete positive integer count"):
            load_csv(fixture.path)

    def test_shared_source_cannot_satisfy_declared_project_gate(self) -> None:
        records = []
        for index in range(1, 13):
            record = _base_record(index, 4.0 + index * 0.45, 5.0 + index * 0.31)
            baseline = int(record["reported_gross_die"])
            record["source_ref"] = "S001"
            record["reported_gross_die"] = str(round(baseline * 1.02))
            records.append(record)
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)
        output = io.StringIO()

        with redirect_stdout(output):
            exit_status = main([str(fixture.path), "--json"])

        report = json.loads(output.getvalue())
        self.assertEqual(exit_status, 2)
        self.assertEqual(report["status"], "invalid_dataset")
        self.assertNotIn("decision", report)
        self.assertNotIn("coefficients", report)
        self.assertTrue(
            any(
                "source S001 maps to multiple project_id values" in error
                for error in report["errors"]
            )
        )

    def test_leave_one_project_out_holds_out_every_row_in_the_project(self) -> None:
        points = [
            Point("P001", 100.0, 200.0),
            Point("P001", 200.0, 400.0),
            Point("P002", 100.0, 100.0),
            Point("P003", 200.0, 200.0),
        ]

        predictions = _cross_validated_predictions(points, "proportional")

        self.assertEqual(predictions[0], (200.0, 100.0))
        self.assertEqual(predictions[1], (400.0, 200.0))

    def test_recommends_proportional_only_after_grouped_cv_gates_pass(self) -> None:
        records = []
        for index in range(1, 13):
            record = _base_record(index, 4.0 + index * 0.45, 5.0 + index * 0.31)
            baseline = int(record["reported_gross_die"])
            record["reported_gross_die"] = str(round(baseline * 1.02))
            records.append(record)
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)

        evaluation = evaluate(load_csv(fixture.path), Gates())

        self.assertEqual(evaluation.decision.status, "calibrated_model_recommended")
        self.assertEqual(evaluation.decision.model, "proportional")
        self.assertIsNotNone(evaluation.decision.coefficients)
        proportional = next(
            result for result in evaluation.models if result.model == "proportional"
        )
        self.assertTrue(proportional.passes_coefficient_gates)

    def test_default_gate_refuses_coefficients_below_twelve_projects(self) -> None:
        records = []
        for index in range(1, 12):
            record = _base_record(index, 4.0 + index * 0.45, 5.0 + index * 0.31)
            baseline = int(record["reported_gross_die"])
            record["reported_gross_die"] = str(round(baseline * 1.02))
            records.append(record)
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)

        evaluation = evaluate(load_csv(fixture.path), Gates())

        self.assertNotEqual(
            evaluation.decision.status,
            "calibrated_model_recommended",
        )
        self.assertIsNone(evaluation.decision.coefficients)
        self.assertTrue(
            all(
                not result.passes_coefficient_gates
                for result in evaluation.models
                if result.model != "identity"
            )
        )

    def test_refuses_coefficients_when_explicit_gates_fail(self) -> None:
        records = []
        adjustments = (-0.06, 0.05, -0.04, 0.06, -0.05, 0.04, -0.06, 0.05)
        for index, adjustment in enumerate(adjustments, start=1):
            record = _base_record(index, 4.2 + index * 0.5, 5.1 + index * 0.27)
            baseline = int(record["reported_gross_die"])
            record["reported_gross_die"] = str(round(baseline * (1.0 + adjustment)))
            records.append(record)
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)

        evaluation = evaluate(load_csv(fixture.path), Gates())

        self.assertEqual(evaluation.decision.status, "no_recommendation")
        self.assertIsNone(evaluation.decision.model)
        self.assertIsNone(evaluation.decision.coefficients)
        self.assertTrue(
            all(
                not result.passes_coefficient_gates
                for result in evaluation.models
                if result.model != "identity"
            )
        )

    def test_identity_is_kept_without_publishing_coefficients(self) -> None:
        records = [
            _base_record(index, 4.0 + index * 0.4, 5.0 + index * 0.3)
            for index in range(1, 9)
        ]
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)

        evaluation = evaluate(load_csv(fixture.path), Gates())

        self.assertEqual(evaluation.decision.status, "geometry_baseline_sufficient")
        self.assertEqual(evaluation.decision.model, "identity")
        self.assertIsNone(evaluation.decision.coefficients)

    def test_cli_json_reports_a_gated_decision(self) -> None:
        records = []
        for index in range(1, 13):
            record = _base_record(index, 4.0 + index * 0.45, 5.0 + index * 0.31)
            baseline = int(record["reported_gross_die"])
            record["reported_gross_die"] = str(round(baseline * 1.02))
            records.append(record)
        fixture = TemporaryCsv(records)
        self.addCleanup(fixture.close)
        output = io.StringIO()

        with redirect_stdout(output):
            exit_status = main([str(fixture.path), "--json"])

        report = json.loads(output.getvalue())
        self.assertEqual(exit_status, 0)
        self.assertEqual(report["decision"]["status"], "calibrated_model_recommended")
        self.assertEqual(report["decision"]["model"], "proportional")
        self.assertIsNotNone(report["decision"]["coefficients"])


if __name__ == "__main__":
    unittest.main()
