# 1ASH external pilot package

This package is for a local pilot rerun, not a leaderboard or GA claim.

## Run

```bash
tar xzf 1ash-pilot-0.1.0.tar.gz
cd 1ash-pilot-0.1.0
python3 -m pip install --user 'inspect-ai==0.3.291'
~/.cache/1ash-venv/bin/python harness/1ash/tests/run_m1.py
~/.cache/1ash-venv/bin/python harness/1ash/suite/run_suite.py
~/.cache/1ash-venv/bin/python harness/1ash/suite/run_safety.py
```

The suite is synthetic: reviewers are placeholders, human time is unmeasured
and no model was scored. An external partner must re-run in its own environment
and report the exact command, image/binary digest, and differences before any
pilot claim is made. `1ash verify` currently means `harness/1ash/verify_report.py`
plus the suite/oracle checks, not a signed third-party report.
