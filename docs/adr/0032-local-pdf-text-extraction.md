# ADR 0032: Local PDF Text Extraction via `pdftotext`

- Status: accepted
- Date: 2026-11-09

## Context

A general agent must read documents, not only UTF-8 text files. PDFs are the
most common format users point at, and 1Agent had no way to extract their text.
Options were:

1. a pure-Rust PDF parser as a new dependency;
2. a hosted multimodal model (network, credentials, uncertain availability);
3. the system `pdftotext` from poppler-utils as a child process.

The project already accepts external host adapters (Skynet Workshop for
terminal sessions, headless Chrome for `browser_read`). A PDF parser is a large
attack surface; keeping it out of process is consistent with that boundary.

## Decision

1. Add a read-only `read_pdf` tool that invokes `pdftotext -layout -enc UTF-8`.
   `PDFTOTEXT_BIN` overrides the binary path.
2. The path must be a regular `.pdf` file inside an allowed read root; symlinks
   are rejected before extraction.
3. Output is capped at 4 MiB read and `max_chars` returned characters (default
   20,000, maximum 100,000). Page form feeds become blank lines.
4. Control characters (including bidi and BOM) cause rejection, matching
   `read_file`; the tool cannot smuggle hidden prompt content.
5. Missing poppler-utils fails with an actionable error. The tool is not
   advertised as working on platforms where it was not verified.
6. The tool is risk `medium`: it parses untrusted local files in a child
   process, though it cannot write or use the network.

## Consequences

- Linux x86_64 with poppler-utils can read text PDFs, including the live test in
  `tests/pdf_live.rs`.
- The extractor is an external runtime dependency; packaging must document it.
- Image-only/scanned PDFs return little or no text. OCR and image understanding
  remain unimplemented and must not be claimed.
- `pdftotext` vulnerabilities are outside the process boundary but still on the
  host; output and path bounds limit impact, not parser memory safety.
- DOCX/PPTX and page-range selection are future work.
