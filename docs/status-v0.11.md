# Status v0.11.0

Date: 2026-11-02
Status: human annotation and inter-rater agreement

## Added since v0.10.0

- `annotate add`, `annotate list` and `annotate agreement`.
- Labels:
  - `supported`;
  - `unsupported`;
  - `needs_evidence`.
- Annotation records include case id, annotator id, evidence sources and note.
- Duplicate case/annotator pairs are rejected.
- Hidden control characters are rejected.
- Agreement report:
  - annotation count;
  - cases with multiple annotations;
  - pair observed agreement;
  - expected agreement;
  - kappa in thousandths;
  - sufficient-data flag.
- Reliability requires at least two independently annotated cases.

## Verification

| Check | Result |
|---|---|
| Unit tests | 49 passed |
| Identical-label kappa | passed |
| Disagreement handling | passed |
| Duplicate annotation rejection | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   79c2b8bcd5f53bd63de6a022db12b61ac2ee6ec170718b5e8becb65d0dd01841
Windows binary: f5a2b1d11fe6380e1ba42055142563a0893b59b50571de07f14c02ffb8eb6d8a
Linux tar.gz:   9ad548af132c6cee4e844b69b14f0fbeb1c3ffa14f409f593bbdd07be885a0b4
Windows zip:    4064016c5388fc058d2f653ca3b1eeca72aca381e5c27cfac094bb3adb87a233
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Redacted human annotation storage | implemented |
| Inter-rater agreement | implemented |
| Case/annotator uniqueness | implemented |
| Evidence-source recording | implemented |
| Confidence calibration | not enabled |
| Real-provider evaluation | blocked by missing credentials |

## Known risks

- Kappa is pairwise for variable rater sets, not a full psychometric agreement suite.
- `needs_data` currently means fewer than two multi-annotated cases; thresholds are not calibrated.
- Annotation identity is a local string and not authenticated.
- Human label collection still requires real reviewers and redaction procedures.

## Next requirement

Add calibration analysis as a separate opt-in report:

1. Compare labels only after agreement reaches a configured threshold.
2. Compute calibration and selective-risk metrics from labeled confidence bins.
3. Refuse numeric model confidence when calibration data is insufficient.
4. Keep confidence out of release gating until evidence supports it.
5. Record the reviewer cohort and label-set digest.
