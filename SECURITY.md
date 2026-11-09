# Security Policy

## Reporting a vulnerability

Do not open a public issue containing exploit details, credentials, memory
contents or journal excerpts. Report privately through the maintainer channel
used to obtain this build, or through the repository's private security advisory
feature when the repository is hosted with one.

Include affected version/commit, a minimal reproduction, impact, and whether the
issue requires a malicious home, skill, provider or local user. Acknowledgement
and remediation timing are best-effort for this pre-release candidate.

## Supported versions

Only the current 0.95.x release-candidate line is supported. There is no
long-term-support branch or production backport commitment yet.

## Security model in brief

- Read-only behavior is the default; tool authorization can require Authority
  and Policy before execution.
- Runtime state is local. Keys, journals and payloads are private files under
  the runtime home; key material is excluded from exports and protected by a
  separate escrow key when escrowed.
- Every material action should be reconstructable from an append-only journal;
  a claimed completion without journal evidence is not a security claim.
- Provider credentials are read from environment variables or hidden CLI input
  and must not be written into configuration, journals or logs.
- Webhook alerts are optional and carry provider/model/failure metadata only.
  Remote webhooks must use HTTPS; loopback HTTP is allowed for local tests.
- Untrusted skills, unknown payload formats and unreviewed provider output are
  untrusted input. Do not mix them with a home whose keys you cannot rotate.

## Explicit non-goals for this candidate

- No HSM/quorum key custody, password KDF, memory zeroization guarantee or
  secure multi-tenant isolation.
- No formal third-party audit yet.
- No guarantee that a backup taken during active writes is a transactional
  point-in-time snapshot.

## Disclosure

Please allow time for a fix and a coordinated release before publishing details.
No warranty is provided; see `LICENSE`.
