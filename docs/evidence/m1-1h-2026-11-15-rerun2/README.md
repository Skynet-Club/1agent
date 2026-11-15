# 0.98 1h M1 — rerun2 (stopped diagnostic)

Date: 2026-11-15  
Model: `deepseek-primary` / `deepseek-flash`  
Binary SHA-256: `493362979b53360814aad40139a66beb28a01126e47b0562e10dd30b9730c54d`  
Status: **stopped after 3 runs / 20 cases** (19 passed, 1 `long-task-plan`
budget_exhausted). This is not a gate result.

The model wrote the requested `plan.md` and then made cosmetic edits and
re-read it, exhausting the 8-step budget before answering. A follow-up prompt
rule now says: "Do not make cosmetic edits (blank lines or spacing) to a
plan/report artifact after writing it, and do not re-read it. If the content
is correct, answer." A one-run smoke after that rule is 8/8.

The final gate evidence is in `../m1-1h-2026-11-15-rerun3/`.
