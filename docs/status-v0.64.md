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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Kind/scope equality is exact; overlapping wildcard scopes are not analyzed.
- Supersession is linear rather than a branching DAG.
- An expired Active record remains stored until explicitly rolled back or superseded.
- Source evidence is bound by run ID and event sequence, not a journal digest.

## Verified clean release

```text
source commit: 2d2e8863de3170a8b0d94c083de7744aff0827f1
git dirty: false

Linux binary:
14cb60913d776859b5b1899658c4319a4299cf628229f580e0c70b71dac3a37a

Windows binary:
7123fc27f2337affb7273108f4f72a2cae2ba18cf6c067b107e8abb39e460d63

Linux tar.gz:
273c22bf29264fa625870af2c576dfc9069ec98afc7d6d51c8c8ff9f9fb1c80f

Windows zip:
8b2ed944a8726d1753d5f036037891078e7a83e5640a6aafb42cedd7bf4567a0

Release manifest:
8061b62acd629b4d56f7a77befe1468b96d90902881690abe0c48d1d6efb6d0b
```

## Next requirement

Bind experience evidence to immutable source content:

1. compute a digest over referenced source events;
2. store the digest in the candidate;
3. revalidate digest before review, approval and activation;
4. reject evidence sequence drift;
5. test rewritten source journals.
