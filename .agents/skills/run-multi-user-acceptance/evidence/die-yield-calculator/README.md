# Die Yield Calculator acceptance evidence

These files record simulated cognitive walkthroughs and automated regression
checks performed while developing the project. They are engineering-process
artifacts, not product documentation and not evidence from human participants.

No participant took part in these rounds. Persona satisfaction is a rubric-based
proxy derived from task completion. Browser interactions were scripted in
isolated profiles, and numerical checks used separate formulas, enumerators, or
invariants. Historical working-tree artifacts are not preserved, so their hashes
record what was tested at the time but do not make those intermediate builds
independently replayable.

The release gate keeps correctness separate from usability: every required
numerical case must pass, even when the simulated task score is otherwise high.
See [acceptance-plan.md](acceptance-plan.md) for the stable gates and
[ux-heuristic-audit.md](ux-heuristic-audit.md) for the project-specific design
review.
