# Agent Instructions

All CLI implementation and CLI UX decisions in this repo must follow these rules:

1. JSON-first contract
- Every command must support machine-readable output.
- Prefer JSON as the default for agent-facing paths.
- Human-friendly text output is secondary and must not change machine semantics.

2. Deterministic interfaces
- Keep flags, field names, and output shapes stable.
- Avoid ambiguous parsing and context-dependent behavior.
- Treat CLI output as an API contract.

3. Non-interactive by default
- No prompts for normal execution paths.
- No hidden defaults that require human interpretation.
- Everything required for execution must be explicit in flags or JSON payloads.

4. Separation of data and presentation
- Do not encode presentation-only strings into machine fields.
- Keep structured data in structured fields; render formatting only in text mode.

5. Script/agent safety
- Commands should be composable, predictable, and idempotent where possible.
- Provide clear exit codes and actionable errors.
- Prefer explicit validation errors over silent coercion.

6. Verb-based task model
- Use clear, action-oriented subcommands (`submit`, `start`, `show`, etc.).
- Keep command behavior narrow and single-purpose.

7. Validation and simulation
- Validate inputs defensively (agents hallucinate malformed values).
- Support dry-run/preview for mutating operations whenever feasible.

8. Runtime semantics
- `tui` is a first-class runtime beside `headless` and `mirrored`.
- Runtime controls task/session execution only. Popup/window presentation belongs in tmux bindings or operator workflow, not in the core CLI contract.

If there is a conflict with existing CLI patterns, align with this rule set.
