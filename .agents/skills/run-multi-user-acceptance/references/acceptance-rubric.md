# Acceptance rubric

## Contents

1. Evidence standard
2. Persona and task design
3. Experience criteria
4. Severity
5. Satisfaction proxy and gates
6. Round JSON format
7. Report structure

## 1. Evidence standard

Use the strongest available evidence:

| Level | Evidence | Permitted conclusion |
|---|---|---|
| A | Reproduced interaction, test output, independent calculation | Observed or verified |
| B | Current screenshot plus matching source code | Strongly supported |
| C | Source code or documentation alone | Inferred behavior |
| D | Assumption without an accessible artifact | Unknown; do not score as pass |

Give every scored task and correctness case at least one concrete evidence item. Include a file and line, URL, screenshot identifier, command output, or compact observation trace.

## 2. Persona and task design

Define personas from workflow differences:

| Persona | Default goal | Main risk |
|---|---|---|
| First-time user | Produce and understand one valid result | Terminology and hidden assumptions |
| Occasional user | Recreate or compare a prior scenario | Recall burden and stale state |
| Domain expert | Control parameters and reproduce a result | Missing controls and opaque models |
| Skeptical verifier | Challenge claims and edge cases | Misleading labels and false precision |
| Access-constrained user | Complete the core path with keyboard, screen reader, or narrow display | Inaccessible or reordered controls |

Use three to seven tasks. Recommended task families:

1. obtain the first valid result;
2. explain the primary outputs in plain language;
3. change one scenario and identify the causal delta;
4. recover from an invalid or ambiguous input;
5. reproduce or export a result with its assumptions;
6. inspect the formula, units, model, and uncertainty;
7. complete the core path in a constrained environment.

Mark a task `core: true` only when failure prevents the persona's primary outcome.

## 3. Experience criteria

Assess each task across these dimensions:

- **Orientation:** Is the next action recognizable without prior memorization?
- **Comprehension:** Can the persona correctly explain the result and its scope?
- **Feedback:** Does the interface expose what changed and why?
- **Recovery:** Can the persona locate and correct an error without losing work?
- **Trust:** Are model, units, defaults, uncertainty, and data origin visible at the right depth?
- **Efficiency:** Can an experienced user avoid repeated navigation and redundant entry?
- **Accessibility:** Are semantics, keyboard operation, contrast, focus, and responsive order usable?

Assign one status:

- `pass`: completes the outcome accurately without material confusion;
- `friction`: completes it, but with avoidable hesitation, excess work, or reduced confidence;
- `fail`: cannot complete it, misunderstands the result, or would plausibly make a wrong decision.

## 4. Severity

- `blocker`: data loss, unsafe action, or broadly wrong primary output.
- `major`: wrong decision, failed core task, silent unit/model error, inaccessible critical path, or materially misleading claim.
- `moderate`: recoverable confusion or repeated friction affecting common work.
- `minor`: localized polish issue with little task impact.

Correctness failures should normally be `blocker` or `major`; do not downgrade them because the numerical difference is small without a domain tolerance.

## 5. Satisfaction proxy and gates

A persona is counted as satisfied when:

- every core task is `pass`; and
- at least 80% of that persona's tasks are `pass`.

The default release gate requires:

- persona satisfaction rate at least 80%;
- task pass rate at least 80%;
- no non-passing novice or domain-expert core task;
- all required correctness cases pass;
- no unresolved blocker or major finding.

This is a structured proxy, not measured human satisfaction. Keep the label `simulated satisfaction` in reports.

## 6. Round JSON format

Use this minimal structure:

```json
{
  "round": "baseline",
  "artifact": {
    "name": "Application name",
    "revision": "commit or build identifier"
  },
  "gates": {
    "minimum_persona_satisfaction": 0.8,
    "minimum_task_pass_rate": 0.8
  },
  "personas": [
    {
      "id": "novice",
      "label": "First-time user",
      "required_core_gate": true,
      "tasks": [
        {
          "id": "first-result",
          "core": true,
          "status": "pass",
          "evidence": ["Observed trace or path:line"],
          "notes": "Optional concise explanation"
        }
      ]
    }
  ],
  "correctness": [
    {
      "id": "known-value",
      "required": true,
      "status": "pass",
      "severity": "major",
      "evidence": ["Independent oracle and observed result"]
    }
  ],
  "findings": [
    {
      "id": "F-001",
      "severity": "moderate",
      "status": "open",
      "summary": "Concise user-facing problem",
      "evidence": ["Observed trace or path:line"]
    }
  ]
}
```

Allowed task statuses are `pass`, `friction`, and `fail`. Allowed correctness statuses are `pass` and `fail`. Allowed finding severities are `blocker`, `major`, `moderate`, and `minor`; finding status is `open` or `resolved`.

## 7. Report structure

Lead with the gate result, then show:

1. scope, revision, environments, and evidence limits;
2. simulated satisfaction, task pass rate, and correctness gate;
3. persona-by-task matrix;
4. correctness cases and independent oracles;
5. open findings ordered by severity;
6. changes and regressions since the prior round;
7. next fixes or remaining need for real-user validation.

Do not bury a failed correctness gate beneath aggregate scores.
