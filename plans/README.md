# plans

Design notes, implementation plans, and dependency evaluations for miniR.

This directory mixes three kinds of documents:

- Active implementation plans for still-open interpreter work
- Dependency and vendor evaluation notes
- Historical snapshots written before later implementation landed

Before acting on a plan, compare it against the current code plus `README.md`, `TODO.md`, and `DONE.md`. Dated "Current State" sections are especially likely to drift.

High-signal plans to start with:

- `implementation-plan.md` — current ordered implementation priorities
- `interpreter-roadmap.md` — high-level roadmap for the remaining compatibility work
- `call-stack.md` — call-frame, `missing()`, and `sys.*` follow-up
- `module-error-types.md` — remaining error-type extraction work
- `error-messages.md` — parser and CLI error-message follow-up, not the original parser rewrite

Most crate-named plans are dependency evaluations. Keep them for integration context, but do not treat them as the current execution order.
