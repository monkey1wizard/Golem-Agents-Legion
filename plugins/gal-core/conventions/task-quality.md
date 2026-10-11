# Task Quality

Canonical questions for checking whether a task is sufficiently specified in its
`Change` and `Acceptance` text. A planner answers every applicable question;
questions that do not apply are not mentioned.

## Questions

1. **Task-spec conformance:** What does this task change, and how does the change and acceptance text cover the task's stated scope?
2. **logical correctness:** What result must the change produce, and what evidence will show that the result is correct?
3. **Boundary conditions:** Which obvious local boundaries or edge cases does the task encounter, and what must happen at each one?
4. **Error handling:** Which failure path does this task introduce or affect, and what does it return or report?
5. **Return values and side effects:** What values, state changes, files, or external effects must the task produce, and which must it avoid?
6. **Layer boundaries:** Which architecture layer owns this change, and how does the task preserve the boundaries between layers?
7. **Existing-pattern consistency:** Which existing pattern or implementation does this task follow?
8. **Abstraction fit (YAGNI):** Why is the proposed abstraction needed, and why is a smaller or existing mechanism insufficient?
9. **Naming conventions:** Which naming rules apply, and what names must the task use?
10. **File and module organization:** Which files or modules change, and why does each belong in this task?
11. **Readability:** What structure, terminology, or explanation is required so the resulting change remains readable?
12. **duplicate code:** Which existing code could be duplicated by this task, and how will duplication be avoided or justified?
13. **dead code:** What does this task remove or update so that no obsolete code, path, or declaration is left unused?
14. **Performance:** Which obvious performance risk does this task introduce or affect, and what keeps it within acceptable bounds?

## Consumers

- **`/refining-plan`:** Writes the answers to the applicable questions into each `T-NN` task's `Change` and `Acceptance` text. Questions that do not apply are not mentioned.
- **ORCHESTRATOR step 2b:** Reads this convention and verifies the task text before writing the cursor. If an applicable question is unanswered, it prints the missing items, instructs the user to rerun `/refining-plan`, and stops without writing the cursor or any other file.
