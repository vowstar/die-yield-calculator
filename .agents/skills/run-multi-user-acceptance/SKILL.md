---
name: run-multi-user-acceptance
description: Run evidence-based simulated multi-user acceptance testing for an application or feature. Use when asked to audit beginner and expert workflows, UX psychology, usability, accessibility, responsiveness, mathematical or domain correctness, acceptance criteria, adversarial testing, or repeated pre/post-change validation. Build independent goal-based personas, test shared tasks, verify calculations against separate oracles and invariants, and gate iterative improvements without presenting simulated users as real research. Especially suited to scientific and engineering calculators such as this die-yield application.
---

# Run Multi-User Acceptance

Evaluate experience and correctness as separate acceptance gates. Treat simulated personas as structured cognitive walkthroughs, never as evidence of real customer sentiment.

## Load the evaluation rules

Read [references/acceptance-rubric.md](references/acceptance-rubric.md) before defining personas, tasks, severities, or a verdict.

For this repository or another numerical die-yield tool, also read [references/die-yield-correctness.md](references/die-yield-correctness.md) before specifying correctness cases.

## 1. Establish scope and evidence

1. Determine whether the request authorizes an audit only or also authorizes implementation. Do not edit during an audit-only request.
2. Record the exact artifact, revision, environment, viewport, and build under test.
3. Prefer a runnable application over screenshots, and screenshots over code inference. Inspect code and tests for claims that cannot be observed from the interface.
4. Label every conclusion as observed behavior, verified calculation, source-backed requirement, or inference.
5. State unavailable evidence. Do not invent interactions, timings, user reactions, calibration data, or browser results.

## 2. Write the acceptance charter

Define:

- the user outcome, not merely the feature list;
- three to seven representative tasks;
- correctness invariants and independent oracle calculations;
- supported environments and accessibility constraints;
- explicit pass gates.

Use these default gates unless the user supplies stricter ones:

- at least 80% of personas are satisfied;
- at least 80% of all persona-task cells pass;
- every core task for the novice and domain-expert personas passes;
- every required correctness case passes;
- no blocker or major unresolved finding remains.

Correctness is a veto gate. Never average a wrong result into an otherwise favorable UX score.

## 3. Build goal-based personas

Choose three to five roles based on distinct knowledge, goals, operating conditions, and failure sensitivity. Avoid demographic stereotypes. Include at minimum:

- a first-time user who wants one valid result and must understand it;
- a domain expert who needs control, traceability, and reproducibility;
- a skeptical verifier who tries to disprove labels, assumptions, and calculations.

Add an occasional user, keyboard/screen-reader user, narrow-screen user, or operations/report consumer when relevant. Do not create multiple personas that exercise the same path.

Give all personas the same core scenario, then add one role-specific task. This makes comparisons meaningful.

## 4. Run independent walkthroughs

When independent agents are available, assign one persona per agent. Give each agent only:

- the raw artifact or exact revision;
- its persona knowledge and goal;
- the task list;
- the evidence format.

Do not disclose other agents' findings, intended fixes, desired verdict, or target score. Assign correctness verification independently from visual review. When agents are unavailable, run the roles serially and explicitly reset assumptions between roles.

For every task, record:

- expected path;
- observed path;
- result: `pass`, `friction`, or `fail`;
- evidence;
- comprehension, recovery, trust, and efficiency notes;
- finding severity when applicable.

Do not claim that a persona felt satisfied. Derive simulated satisfaction from the rubric and name it as a proxy.

## 5. Verify correctness separately

Create an oracle that does not call the production calculation being tested. Use hand-derived values, a small independent calculation, published primary formulas, known fixtures, or property-based invariants.

Test at least:

- representative known values;
- zero and near-zero behavior;
- valid limits and invalid values;
- unit conversion and rounding;
- discrete-boundary behavior;
- stale-state and reset behavior;
- export/report parity when applicable;
- native/browser parity when both are supported.

For fitted models, verify data provenance, separation of training and validation, out-of-domain warnings, model versioning, and prediction intervals. Do not approve a fitted coefficient without the underlying dataset and validation evidence.

## 6. Synthesize without voting away risk

Merge duplicate observations only after the independent passes finish. Preserve disagreements and explain whether they arise from knowledge level, environment, or ambiguous evidence.

Prioritize findings by user harm:

1. wrong or misleading decisions;
2. blocked core tasks and inaccessible controls;
3. hidden assumptions and poor recovery;
4. avoidable expert friction;
5. cosmetic polish.

Write the smallest coherent improvement plan that addresses observed failures. Tie every proposed change to at least one failed task, correctness case, or explicit requirement.

## 7. Iterate and attack

For an implementation request, use at least three rounds unless the user limits scope:

1. **Baseline:** establish failures before editing.
2. **Acceptance rerun:** repeat the same personas and tasks after focused changes.
3. **Adversarial regression:** challenge units, extremes, ambiguous terminology, hidden defaults, stale state, narrow layouts, keyboard access, exports, and model-domain limits.

Keep scenarios and gates stable between rounds. Add adversarial cases; do not silently remove failing cases. Stop only when the gates pass or a genuine external dependency is missing. Report a blocker instead of weakening a gate.

## 8. Score and report

Store each round in the JSON format defined by the rubric. Run:

```sh
python3 .agents/skills/run-multi-user-acceptance/scripts/evaluate_acceptance.py <round.json>
```

Treat a nonzero exit status as a failed gate. Use `--json` for machine-readable output.

Report:

- artifact and environments tested;
- simulated-persona satisfaction and task pass rates;
- correctness gate result;
- novice and expert core-task results;
- evidence-backed findings by severity;
- changes between rounds;
- unresolved risks and what still requires real-user research.

Never describe the result as validated by real users unless real participants actually performed the study.
