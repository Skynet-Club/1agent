# ADR 0033: Skill Bundles and Skill-Bound Routines

- Status: accepted
- Date: 2026-11-09

## Context

Skills lived in a local journal and could be listed/evaluated/activated through
tools, but there was no way to move an immutable set of versions between
machines, and no way to schedule a task against a specific approved version.
The roadmap calls for a plugin/skill marketplace and Routine semantics.

A full marketplace needs a remote index, public-key signatures and revocation.
Those pieces do not exist yet. Shipping only a local distribution primitive
must not be described as a marketplace.

## Decision

1. `skill export` writes a versioned JSON bundle of immutable manifests.
   `skill import` requires the caller-supplied SHA-256 of the whole bundle.
2. Import rejects size > 4 MiB, unknown schemas, empty bundles, duplicate
   `name@version`, mismatched per-manifest body hashes and any manifest that
   fails normal skill validation/scanning. All manifests are preflighted in a
   throwaway `SkillStore` before the journal is touched, so a bad unit cannot
   leave a half-applied import.
3. Existing versions are skipped, not overwritten; versions are immutable.
4. `run --routine-skill NAME@VERSION` and `schedule add --routine-skill` compose
   the body of the currently active, exact version into the task. An inactive or
   mismatched version is rejected.
5. A routine is not a permission source: it gains no tools, no write authority
   and no network access beyond the run's existing permissions.
6. Bundles may carry an Ed25519 signature. `skill keygen` writes hex PKCS#8
   private and raw public keys as 0600 files; export signs the canonical
   `{schema_version, skills}` payload, and import requires the trusted public
   key for signed bundles. The embedded key must match the trusted key. A
   signed bundle without a key, an unsigned bundle with a key, and a tampered
   signature all fail closed.
7. A remote marketplace index, revocation list and key discovery remain future
   work. The bundle hash is still required, so integrity and provenance are
   separate gates.

## Consequences

- Two machines can exchange an exact skill set with a hash that is easy to
  publish out of band.
- The operator must obtain the bundle hash through a trusted channel; a bundle
  served over HTTPS alone is not end-to-end authenticated.
- Ed25519 signing lets an importer verify provenance once the public key is
  trusted out of band. There is no key directory, revocation or expiry yet.
- A routine's composed objective is journaled verbatim, so it is replayable but
  may be longer than the original task.
- Changing a skill requires registering a new version and activating it; a
  routine pinned to an old version fails closed after activation changes.
