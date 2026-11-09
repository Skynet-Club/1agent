# Development Flywheel

Every iteration follows the same gates:

1. Read the requirement and record assumptions.
2. Compare architecture and technical options.
3. Implement the smallest complete vertical slice.
4. Run formatting, unit tests, integration tests, lint and the claims check
   (`scripts/verify.sh`). A scheduled `scripts/ci.sh` run records an evidence line
   and a binary digest outside the repository.
5. Verify behavior with a real executable or environment check.
6. Audit journal, permissions and evidence invariants.
7. Compare the result with the requirement.
8. Update architecture, ADRs, README and the Obsidian `1-Agent` document.
9. Record unresolved items and select the next requirement.

No phase is complete if its only evidence is model self-assessment.
