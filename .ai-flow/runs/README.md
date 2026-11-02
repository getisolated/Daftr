# Runs of the AI flow

Each flow works on its own branch `ai/<n>-<slug>` and writes its shared memory in
`.ai-flow/runs/<n>/` (`<n>` = issue number). This folder stays empty on `main`; on a flow
branch it holds:

| File | Written by | Content |
| --- | --- | --- |
| `spec.md` | specs | Final spec: goal, scope, numbered acceptance criteria (French) |
| `plan.md` | plan | Development plan for the next agents, plus "Retour PM, tour N" sections |
| `tests.md` | tests | Each test and the acceptance criterion it covers |
| `code-notes.md` | code | Deviations from the plan, justified `#[allow]`, new dependencies |
| `review.md` | review | Verdict `APPROUVÉ` / `RETOUR CODE`, findings, loop counter |
| `rapport-pm.md` | review, deploy | Report for the PM, non-technical (French) |

The git history of these files is the audit trail of the flow.
