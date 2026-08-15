#!/usr/bin/env python3
"""Evaluate evidence-backed simulated acceptance gates from one JSON round."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


TASK_STATUSES = {"pass", "friction", "fail"}
CORRECTNESS_STATUSES = {"pass", "fail"}
SEVERITIES = {"blocker", "major", "moderate", "minor"}


class InputError(ValueError):
    """Raised when an acceptance report does not satisfy the minimal schema."""


def require_evidence(item: dict[str, Any], location: str) -> None:
    evidence = item.get("evidence")
    if not isinstance(evidence, list) or not evidence or not all(
        isinstance(entry, str) and entry.strip() for entry in evidence
    ):
        raise InputError(f"{location}.evidence must be a non-empty string list")


def fraction(value: Any, location: str) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise InputError(f"{location} must be a number")
    result = float(value)
    if not 0.0 <= result <= 1.0:
        raise InputError(f"{location} must be between 0 and 1")
    return result


def evaluate(report: dict[str, Any]) -> dict[str, Any]:
    personas = report.get("personas")
    correctness = report.get("correctness")
    if not isinstance(personas, list) or not personas:
        raise InputError("personas must be a non-empty list")
    if not isinstance(correctness, list) or not correctness:
        raise InputError("correctness must be a non-empty list")

    gates = report.get("gates", {})
    if not isinstance(gates, dict):
        raise InputError("gates must be an object")
    minimum_personas = fraction(gates.get("minimum_persona_satisfaction", 0.8), "gates.minimum_persona_satisfaction")
    minimum_tasks = fraction(gates.get("minimum_task_pass_rate", 0.8), "gates.minimum_task_pass_rate")

    total_tasks = 0
    passed_tasks = 0
    satisfied_personas = 0
    core_gate_failures: list[str] = []

    for persona_index, persona in enumerate(personas):
        location = f"personas[{persona_index}]"
        if not isinstance(persona, dict):
            raise InputError(f"{location} must be an object")
        persona_id = persona.get("id")
        tasks = persona.get("tasks")
        if not isinstance(persona_id, str) or not persona_id.strip():
            raise InputError(f"{location}.id must be a non-empty string")
        if not isinstance(tasks, list) or not tasks:
            raise InputError(f"{location}.tasks must be a non-empty list")

        persona_passed = 0
        persona_core_passed = True
        for task_index, task in enumerate(tasks):
            task_location = f"{location}.tasks[{task_index}]"
            if not isinstance(task, dict):
                raise InputError(f"{task_location} must be an object")
            task_id = task.get("id")
            status = task.get("status")
            if not isinstance(task_id, str) or not task_id.strip():
                raise InputError(f"{task_location}.id must be a non-empty string")
            if status not in TASK_STATUSES:
                raise InputError(f"{task_location}.status must be one of {sorted(TASK_STATUSES)}")
            require_evidence(task, task_location)
            total_tasks += 1
            if status == "pass":
                passed_tasks += 1
                persona_passed += 1
            if bool(task.get("core", False)) and status != "pass":
                persona_core_passed = False
                if bool(persona.get("required_core_gate", False)):
                    core_gate_failures.append(f"{persona_id}/{task_id}")

        if persona_core_passed and persona_passed / len(tasks) >= 0.8:
            satisfied_personas += 1

    correctness_failures: list[str] = []
    for case_index, case in enumerate(correctness):
        location = f"correctness[{case_index}]"
        if not isinstance(case, dict):
            raise InputError(f"{location} must be an object")
        case_id = case.get("id")
        status = case.get("status")
        severity = case.get("severity")
        if not isinstance(case_id, str) or not case_id.strip():
            raise InputError(f"{location}.id must be a non-empty string")
        if status not in CORRECTNESS_STATUSES:
            raise InputError(f"{location}.status must be one of {sorted(CORRECTNESS_STATUSES)}")
        if severity not in SEVERITIES:
            raise InputError(f"{location}.severity must be one of {sorted(SEVERITIES)}")
        require_evidence(case, location)
        if bool(case.get("required", True)) and status != "pass":
            correctness_failures.append(case_id)

    severe_findings: list[str] = []
    findings = report.get("findings", [])
    if not isinstance(findings, list):
        raise InputError("findings must be a list")
    for finding_index, finding in enumerate(findings):
        location = f"findings[{finding_index}]"
        if not isinstance(finding, dict):
            raise InputError(f"{location} must be an object")
        finding_id = finding.get("id")
        severity = finding.get("severity")
        status = finding.get("status")
        if not isinstance(finding_id, str) or not finding_id.strip():
            raise InputError(f"{location}.id must be a non-empty string")
        if severity not in SEVERITIES:
            raise InputError(f"{location}.severity must be one of {sorted(SEVERITIES)}")
        if status not in {"open", "resolved"}:
            raise InputError(f"{location}.status must be open or resolved")
        require_evidence(finding, location)
        if status == "open" and severity in {"blocker", "major"}:
            severe_findings.append(finding_id)

    persona_rate = satisfied_personas / len(personas)
    task_rate = passed_tasks / total_tasks
    reasons: list[str] = []
    if persona_rate < minimum_personas:
        reasons.append("simulated persona satisfaction is below the configured threshold")
    if task_rate < minimum_tasks:
        reasons.append("persona-task pass rate is below the configured threshold")
    if core_gate_failures:
        reasons.append("required core tasks did not pass")
    if correctness_failures:
        reasons.append("required correctness cases failed")
    if severe_findings:
        reasons.append("blocker or major findings remain open")

    return {
        "verdict": "pass" if not reasons else "fail",
        "round": report.get("round", "unspecified"),
        "persona_satisfaction_rate": persona_rate,
        "satisfied_personas": satisfied_personas,
        "total_personas": len(personas),
        "task_pass_rate": task_rate,
        "passed_tasks": passed_tasks,
        "total_tasks": total_tasks,
        "core_gate_failures": core_gate_failures,
        "correctness_failures": correctness_failures,
        "open_severe_findings": severe_findings,
        "reasons": reasons,
    }


def render_markdown(result: dict[str, Any]) -> str:
    verdict = result["verdict"].upper()
    lines = [
        f"# Acceptance gate: {verdict}",
        "",
        f"- Round: {result['round']}",
        (
            "- Simulated persona satisfaction: "
            f"{result['satisfied_personas']}/{result['total_personas']} "
            f"({result['persona_satisfaction_rate']:.1%})"
        ),
        (
            "- Persona-task pass rate: "
            f"{result['passed_tasks']}/{result['total_tasks']} "
            f"({result['task_pass_rate']:.1%})"
        ),
        f"- Required core-task failures: {len(result['core_gate_failures'])}",
        f"- Required correctness failures: {len(result['correctness_failures'])}",
        f"- Open blocker/major findings: {len(result['open_severe_findings'])}",
    ]
    if result["reasons"]:
        lines.extend(["", "## Failed gates", ""])
        lines.extend(f"- {reason}" for reason in result["reasons"])
    return "\n".join(lines)


def self_test() -> None:
    passing = {
        "round": "self-test-pass",
        "personas": [
            {
                "id": role,
                "required_core_gate": role in {"novice", "expert"},
                "tasks": [
                    {"id": "core", "core": True, "status": "pass", "evidence": ["fixture"]},
                    {"id": "secondary", "core": False, "status": "pass", "evidence": ["fixture"]},
                ],
            }
            for role in ["novice", "occasional", "expert", "verifier", "access"]
        ],
        "correctness": [
            {"id": "oracle", "required": True, "status": "pass", "severity": "major", "evidence": ["fixture"]}
        ],
        "findings": [],
    }
    assert evaluate(passing)["verdict"] == "pass"
    failing = json.loads(json.dumps(passing))
    failing["round"] = "self-test-fail"
    failing["personas"][0]["tasks"][0]["status"] = "friction"
    failing["correctness"][0]["status"] = "fail"
    assert evaluate(failing)["verdict"] == "fail"
    print("self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", nargs="?", type=Path, help="acceptance round JSON")
    parser.add_argument("--json", action="store_true", help="emit machine-readable result")
    parser.add_argument("--self-test", action="store_true", help="run built-in tests")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0
    if args.report is None:
        parser.error("report is required unless --self-test is used")

    try:
        report = json.loads(args.report.read_text(encoding="utf-8"))
        if not isinstance(report, dict):
            raise InputError("top-level JSON value must be an object")
        result = evaluate(report)
    except (OSError, json.JSONDecodeError, InputError) as error:
        print(f"invalid acceptance report: {error}", file=sys.stderr)
        return 2

    print(json.dumps(result, indent=2) if args.json else render_markdown(result))
    return 0 if result["verdict"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
