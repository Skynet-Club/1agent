# read_pdf live extraction — 2026-11-09

Proves that `read_pdf` extracts text from a real PDF through the system
`pdftotext`, not only through a fake extractor.

## Environment

- Linux x86_64
- `pdftotext` 26.01.0 (poppler)
- Ghostscript `ps2pdf` used only to generate the fixture

## Reproduction

```bash
# Requires poppler-utils and ghostscript.
cargo test --offline --test pdf_live -- --ignored
```

Observed:

```text
running 1 test
test real_pdftotext_extracts_text_from_a_generated_pdf ... ok
```

The test writes a one-page PostScript fixture, converts it to PDF, calls
`PdfReadTool::new("pdftotext")` with a read root, and asserts the extracted text
contains `Hello 1A PDF`.

## Unit coverage

```bash
cargo test --offline --lib tools::documents
```

Covers: text extraction with a fake extractor, character truncation, read-root
containment, symlink rejection, non-PDF rejection, missing extractor and
control-character rejection. Page form feeds are normalized to blank lines.

## Not claimed

- No image, scan or OCR understanding.
- No DOCX/PPTX extraction.
- No page-range selection; `max_chars` bounds the returned text.
- Only the Linux `pdftotext` backend is verified.
