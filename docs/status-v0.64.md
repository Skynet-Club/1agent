# Status v0.64.0

Date: 2026-11-02
Status: atomic experience supersession implemented

## Changes since v0.63.0

- Experience adds `superseded`.
- A superseding candidate must reference an Active experience.
- Superseding kind and scope must match exactly.
- Candidate version is the superseded version plus one.
- Activation atomically changes:
  - prior experience `active -> superseded`;
  - replacement `approved -> active`.
- Only one Active experience may exist for a given kind and scope.
- Expired candidates cannot be proposed or activated.
- Rolling back a replacement does not reactivate or erase the superseded record.

## Verification

| Check | Result |
|---|---|
| Unit tests | 110 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| competing Active blocked | passed |
| atomic supersession | passed |
| version increment | passed |
| kind mismatch | rejected |
| scope mismatch | rejected |
| non-Active supersession target | rejected |
| expired proposal | rejected |
| expired activation | rejected |
| rollback preserves prior history | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Kind/scope equality is exact; overlapping wildcard scopes are not analyzed.
- Supersession is linear rather than a branching DAG.
- An expired Active record remains stored until explicitly rolled back or superseded.
- Source evidence is bound by run ID and event sequence, not a journal digest.

## Next requirement

Bind experience evidence to immutable source content:

1. compute a digest over referenced source events;
2. store the digest in the candidate;
3. revalidate digest before review, approval and activation;
4. reject evidence sequence drift;
5. test rewritten source journals.
