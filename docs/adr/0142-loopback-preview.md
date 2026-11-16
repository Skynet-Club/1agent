# ADR 0142: Loopback Preview for Generated Pages

- Status: accepted
- Date: 2026-11-16
- Related: ADR-0141, `docs/manual.md`, `docs/features.md`

## Context

A website task wrote a complete single-file page and returned a `conditional`
answer because `terminal_create` failed with `session_limit` (128 active
sessions). The answer told the user to run `python3 -m http.server` themselves.
The page was correct on disk, but the user could not click a URL. A second
weakness was the verification: the model re-read only 1200 bytes of a 13,779
byte file, which is not whole-artifact verification.

The existing terminal tools require a PTY session and are not a reliable
preview path when the session daemon is saturated. A page preview must not
depend on the user starting a server.

## Decision

1. Add `preview_file`, a read-only tool that serves one regular file or a
   directory inside an allowed read root on `127.0.0.1` (ephemeral port by
   default).
2. The tool performs its own `GET /` and returns only after an HTTP 200 with a
   non-empty body. The observation includes the URL, HTTP status, byte count
   and SHA-256; the URL is the clickable receipt.
3. The server is loopback-only, rejects symlinks and path traversal, caps a
   served file at 16 MiB and keeps at most eight preview servers. It lives only
   while the 1Agent process runs; this is an interactive preview, not a public
   server.
4. The full-Agent system prompt requires website tasks to use `preview_file`
   and give the returned URL. It must not tell the user to start a server. If
   preview fails, the answer must be `conditional` with the exact blocker and
   a `file://` path; it must not claim the page is accessible.
5. The prompt also states that a partial `read_file` (`max_bytes`) is not
   whole-artifact verification; full bytes or the write receipt hash are
   required.

## Consequences

- A generated single-file page can be opened by clicking a local URL while the
  TUI process is running; the served directory also supports relative assets.
- The preview disappears when the 1Agent process exits. A durable preview
  daemon, a browser auto-open action and a per-user port registry are future
  work.
- The tool is read-only and available in read-only runs, but it opens a
  loopback listener. It does not expose a non-loopback interface.
- A full 128-session terminal does not block page preview.
- The model can still choose not to call the tool; the prompt is a policy
  instruction, not a hard runtime gate. A hard gate for "website" tasks would
  need task classification and should be a separate slice.

## Evidence

```bash
cargo test --lib tools::preview
cargo test --lib system_prompt_requires_a_verified_preview_for_pages
```

The tests cover file serving, HTTP 200 verification, read-root rejection,
directory index serving and traversal rejection. They do not prove a real
browser render; that remains a manual visual check.
