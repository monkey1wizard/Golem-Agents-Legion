---
name: benchmark-checklist
description: Check whether performance measurements support a decision and report comparable results.
---

# Benchmark Checklist

State the decision first. Before using a measurement to choose a design or judge a performance budget, answer all seven questions below. Record the observation and supporting evidence for each answer, or record the gap. Do not treat an unmeasured assumption as a result.

1. **Limiter:** What resource or operation limits each result, and what evidence identifies it? Check for changes in the limiter between sides.
2. **Production settings:** Do both sides use comparable production settings, including build mode, configuration, concurrency, data, and environment? Record differences and their effect on comparability.
3. **Bounds:** What hardware capacity or theoretical bound constrains the result? Compare the measured rate or time with that bound and investigate implausible results.
4. **Errors and correctness:** Did every measured run complete without errors and produce the correct output? Verify outputs against an independent expectation and record failures.
5. **Repeatability:** Do repeated runs under the same conditions produce consistent results? Record the number of runs and variation for each side.
6. **User path:** Does the workload represent the user path and input sizes relevant to the decision? Record any differences from production use.
7. **Timed work:** What work is inside and outside the timed region? Confirm that both sides complete equivalent work, including setup, cleanup, and asynchronous work where relevant.

## Decision and report

Report `inconclusive` and withhold the decision if settings are incomparable, the limiter is unknown, or errors, output correctness, or completed work are unverified. Record other gaps and their effect on the decision. Do not convert missing evidence into a pass.

Lead with the decision or `inconclusive`, followed by the target or budget and the following for each side:

- Units and number of measured runs, excluding warmups.
- Median: sort the measurements and take the middle value, or the mean of the two middle values for an even number of runs.
- Range: the lowest and highest measurements in the same units.
- Limiter and evidence identifying it.
- Reproduction conditions: command, versions, hardware, operating system, build and configuration, workload, concurrency, warmups, and output verification.

When comparing against an explicit budget, report the measured result and delta against that budget. A number without a budget is not a pass or fail verdict. Describe the scope and any remaining uncertainty.

## Equivalent command examples

These examples run the same two hypothetical executables with the same input, warmups, and measured run count. Install `hyperfine` and substitute real commands and input. Verify correct output and equivalent completed work separately. The commands do not establish the limiter or production relevance on their own.

Windows PowerShell:

```powershell
hyperfine --warmup 3 --runs 10 '.\baseline.exe input.dat' '.\candidate.exe input.dat'
```

Linux shell:

```sh
hyperfine --warmup 3 --runs 10 './baseline input.dat' './candidate input.dat'
```

Use the individual measured run times to calculate the median and range in consistent units. Record the actual executable paths, versions, and environment with the results.
