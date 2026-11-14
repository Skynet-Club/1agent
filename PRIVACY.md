# Privacy and Data Handling

1Agent is local-first. It does not include telemetry, analytics or a mandatory
cloud account.

## What stays local

- Task journals, task graph, goals, commitments, skills, delegation and
  evolution snapshots.
- Memory metadata and encrypted payloads, the master key and the KEK ledger.
- Audit records, action logs, alerts and run history.

The default runtime home is `$ONEAGENT_HOME` or `$HOME/.1a`. Directories are
private (0700) and key files are 0600.

## What leaves the machine

- **Model providers**: prompts, tool observations and task context are sent only
  to the provider you configure. Provider credentials are read from environment
  variables or hidden CLI input.
- **Alert webhook**: if you set `--alert-webhook`, validated alert records
  (provider id, model, failure count, threshold, message, timestamp) are POSTed
  to that endpoint. No API keys, headers or prompt content are included.
- **Workshop**: if you connect Workshop, execution metadata and artifacts are
  governed by that integration.

## Journals and exports

Journals may contain task text, tool observations and final answers. Treat them
as sensitive. Memory exports exclude key material and credential-looking files;
they are not encrypted by the export command itself. Key escrow is encrypted and
must be stored apart from its recovery key.

## Retention and deletion

Soft/hard forget controls encrypted memory. `uninstall` keeps data by default;
`uninstall --purge --yes` deletes the runtime home. Automatic retention policy is
not implemented in this candidate.

## Contact

Use the private maintainer channel for privacy questions or data-handling
incidents. Do not include secrets in reports.
