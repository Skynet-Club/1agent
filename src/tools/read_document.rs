//! One read-only entry point for local documents.
//!
//! Plain text is read directly; PDF goes through `pdftotext`; OOXML/ODF/EPUB
//! are ZIP containers whose XML is reduced to text; HTML/XML are tag-stripped;
//! images use `tesseract` when it is installed; legacy Office formats use
//! `catdoc`/`xls2csv`/`catppt` when available. Missing external backends fail
//! with an explicit install hint instead of returning binary noise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::safety::contains_disallowed_control;

use super::{
    AgentTool, PdfReadTool, ToolError, ToolOutcome, find_containing_root, spawn_with_retry,
};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_MAX_CHARS: usize = 20_000;
const MAX_MAX_CHARS: usize = 200_000;
const MAX_ARCHIVE_ENTRIES: usize = 200;
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct DocumentBinaries {
    pub pdf: PathBuf,
    pub unzip: PathBuf,
    pub tesseract: PathBuf,
    /// Optional Python interpreter with `rapidocr_onnxruntime` installed.
    pub ocr_python: Option<PathBuf>,
    pub catdoc: PathBuf,
    pub xls2csv: PathBuf,
    pub catppt: PathBuf,
    pub sqlite3: PathBuf,
    pub tar: PathBuf,
}

impl DocumentBinaries {
    pub fn from_env() -> Self {
        fn env(name: &str, default: &str) -> PathBuf {
            std::env::var_os(name)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(default))
        }
        let ocr_python = std::env::var_os("OCR_PYTHON")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local/share/1agent-ocr/bin/python"))
                    .filter(|path| path.is_file())
            });
        Self {
            pdf: env("PDFTOTEXT_BIN", "pdftotext"),
            unzip: env("UNZIP_BIN", "unzip"),
            tesseract: env("TESSERACT_BIN", "tesseract"),
            ocr_python,
            catdoc: env("CATDOC_BIN", "catdoc"),
            xls2csv: env("XLS2CSV_BIN", "xls2csv"),
            catppt: env("CATPPT_BIN", "catppt"),
            sqlite3: env("SQLITE3_BIN", "sqlite3"),
            tar: env("TAR_BIN", "tar"),
        }
    }
}

impl Default for DocumentBinaries {
    fn default() -> Self {
        Self::from_env()
    }
}

pub struct ReadDocumentTool {
    bins: DocumentBinaries,
}

impl ReadDocumentTool {
    pub fn new(bins: DocumentBinaries) -> Self {
        Self { bins }
    }

    pub fn from_env() -> Self {
        Self::new(DocumentBinaries::from_env())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadDocumentArgs {
    path: String,
    #[serde(default = "default_max_chars")]
    max_chars: usize,
}

fn default_max_chars() -> usize {
    DEFAULT_MAX_CHARS
}

#[async_trait]
impl AgentTool for ReadDocumentTool {
    fn parallel_safe(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_document".into(),
            description: "Read a local document inside an allowed read root. Supports text/code/markup, HTML/XML, PDF, DOCX, PPTX, XLSX, ODT/ODS/ODP, EPUB, RTF, CSV/TSV, EML, IPYNB, image OCR (tesseract), legacy DOC/XLS/PPT (catdoc/xls2csv/catppt), and archive entry listings. Read-only; missing converters fail with an install hint.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path"],
                "properties": {
                    "path": {"type": "string"},
                    "max_chars": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_MAX_CHARS,
                        "default": DEFAULT_MAX_CHARS
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn timeout(&self) -> Duration {
        TOOL_TIMEOUT
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: ReadDocumentArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if args.max_chars == 0 || args.max_chars > MAX_MAX_CHARS {
            return Err(ToolError::InvalidArguments(format!(
                "max_chars must be between 1 and {MAX_MAX_CHARS}"
            )));
        }
        let (canonical, root, bytes) = resolve_document_path(&args.path, permissions).await?;
        if cancellation.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let relative = canonical
            .strip_prefix(&root)
            .unwrap_or(canonical.as_path())
            .to_string_lossy()
            .to_string();
        let format = canonical
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if format == "pdf" {
            let outcome = PdfReadTool::new(self.bins.pdf.clone())
                .execute(
                    json!({"path": canonical.display().to_string(), "max_chars": args.max_chars}),
                    permissions,
                    cancellation,
                )
                .await?;
            let mut metadata = outcome.metadata;
            metadata.insert("format".into(), "pdf".into());
            metadata.insert("extractor".into(), "pdftotext".into());
            metadata.insert("bytes".into(), bytes.to_string());
            metadata.insert("relative_path".into(), relative.clone());
            return Ok(ToolOutcome {
                summary: format!("PDF: {}", outcome.summary),
                content: outcome.content,
                metadata,
            });
        }

        let (content, extractor) = match format.as_str() {
            "docx" => (
                extract_zip_text(
                    &self.bins.unzip,
                    &canonical,
                    &["word/document.xml", "word/header", "word/footer"],
                    true,
                )
                .await?,
                "unzip+ooxml",
            ),
            "pptx" => (
                extract_pptx(&self.bins.unzip, &canonical).await?,
                "unzip+ooxml",
            ),
            "xlsx" => (
                extract_xlsx(&self.bins.unzip, &canonical).await?,
                "unzip+ooxml",
            ),
            "odt" | "ods" | "odp" => (
                extract_zip_text(&self.bins.unzip, &canonical, &["content.xml"], true).await?,
                "unzip+odf",
            ),
            "epub" => (
                extract_epub(&self.bins.unzip, &canonical).await?,
                "unzip+epub",
            ),
            "html" | "htm" | "xhtml" => (strip_markup(&read_utf8(&canonical).await?), "html-text"),
            "xml" | "svg" => (read_utf8(&canonical).await?, "xml-text"),
            "rtf" => (strip_rtf(&read_utf8(&canonical).await?), "rtf-text"),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" => (
                ocr_image(
                    &self.bins.tesseract,
                    self.bins.ocr_python.as_deref(),
                    &canonical,
                    cancellation,
                )
                .await?,
                "ocr",
            ),
            "doc" => (
                run_text(
                    &self.bins.catdoc,
                    &[canonical.as_path()],
                    "catdoc (apt install catdoc)",
                )
                .await?,
                "catdoc",
            ),
            "xls" => (
                run_text(
                    &self.bins.xls2csv,
                    &[canonical.as_path()],
                    "xls2csv (apt install catdoc)",
                )
                .await?,
                "xls2csv",
            ),
            "ppt" => (
                run_text(
                    &self.bins.catppt,
                    &[canonical.as_path()],
                    "catppt (apt install catdoc)",
                )
                .await?,
                "catppt",
            ),
            "sqlite" | "db" => (
                run_text(
                    &self.bins.sqlite3,
                    &[canonical.as_path(), Path::new(".tables")],
                    "sqlite3 (apt install sqlite3)",
                )
                .await?,
                "sqlite3",
            ),
            "zip" => (list_zip(&self.bins.unzip, &canonical).await?, "unzip-list"),
            "tar" | "tgz" | "gz" => (list_tar(&self.bins.tar, &canonical).await?, "tar-list"),
            "mp3" | "wav" | "m4a" | "flac" | "ogg" | "mp4" | "mov" | "mkv" => {
                return Err(ToolError::Failed(
                    "audio/video transcription requires a whisper CLI; install whisper.cpp or set a transcription backend"
                        .into(),
                ));
            }
            "txt" | "md" | "markdown" | "rst" | "log" | "csv" | "tsv" | "json" | "jsonl"
            | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "eml" | "ipynb" | "tex"
            | "sql" | "sh" | "py" | "rs" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c"
            | "cc" | "cpp" | "h" | "hpp" | "css" | "scss" | "less" | "srt" | "vtt" => {
                (read_utf8(&canonical).await?, "text")
            }
            _ => match read_utf8(&canonical).await {
                Ok(text) => (text, "text"),
                Err(_) => {
                    return Err(ToolError::Failed(format!(
                        "unsupported document format '{format}'; supported: text, html, xml, pdf, docx, pptx, xlsx, odt/ods/odp, epub, rtf, csv/tsv, images, doc/xls/ppt (catdoc), zip/tar"
                    )));
                }
            },
        };

        if contains_disallowed_control(&content) {
            return Err(ToolError::UnsafeContent(
                "document text contains hidden or bidirectional control characters".into(),
            ));
        }
        let char_count = content.chars().count();
        let truncated = char_count > args.max_chars;
        let content = if truncated {
            let mut bounded: String = content.chars().take(args.max_chars).collect();
            bounded.push_str("…[truncated]");
            bounded
        } else {
            content
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("format".into(), format);
        metadata.insert("extractor".into(), extractor.into());
        metadata.insert("bytes".into(), bytes.to_string());
        metadata.insert("chars".into(), content.chars().count().to_string());
        metadata.insert("truncated".into(), truncated.to_string());
        metadata.insert("relative_path".into(), relative.clone());
        Ok(ToolOutcome {
            summary: format!(
                "read {} as {} ({} chars{})",
                relative,
                extractor,
                content.chars().count(),
                if truncated { ", truncated" } else { "" }
            ),
            content,
            metadata,
        })
    }
}

async fn resolve_document_path(
    path: &str,
    permissions: &PermissionSet,
) -> Result<(PathBuf, PathBuf, u64), ToolError> {
    if path.trim().is_empty() {
        return Err(ToolError::InvalidArguments("path must not be empty".into()));
    }
    if permissions.read_roots.is_empty() {
        return Err(ToolError::PermissionDenied(
            "no read roots configured".into(),
        ));
    }
    let requested = PathBuf::from(path);
    let requested = if requested.is_absolute() {
        requested
    } else {
        permissions.read_roots[0].join(requested)
    };
    let metadata = tokio::fs::symlink_metadata(&requested)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot inspect file path: {error}")))?;
    if metadata.file_type().is_symlink() {
        return Err(ToolError::PermissionDenied(
            "file path must not be a symbolic link".into(),
        ));
    }
    let canonical = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot resolve file: {error}")))?;
    let root = find_containing_root(&canonical, &permissions.read_roots).ok_or_else(|| {
        ToolError::PermissionDenied("path is outside configured read roots".into())
    })?;
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot read metadata: {error}")))?;
    if !metadata.is_file() {
        return Err(ToolError::Failed("path is not a regular file".into()));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(ToolError::Failed(format!(
            "document exceeds {MAX_FILE_BYTES} bytes"
        )));
    }
    Ok((canonical, root, metadata.len()))
}

async fn read_utf8(path: &Path) -> Result<String, ToolError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot open file: {error}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot read file: {error}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ToolError::Failed(format!(
            "document exceeds {MAX_FILE_BYTES} bytes"
        )));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| ToolError::Failed("file is not valid UTF-8".into()))?;
    if contains_disallowed_control(&text) {
        return Err(ToolError::UnsafeContent(
            "file contains hidden or bidirectional control characters".into(),
        ));
    }
    Ok(text)
}

async fn run_output(
    binary: &Path,
    args: &[&Path],
    hint: &str,
) -> Result<Option<Vec<u8>>, ToolError> {
    let mut command = Command::new(binary);
    command
        .args(args.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = match spawn_with_retry(&mut command).await {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ToolError::Failed(format!(
                "cannot start {}: {error}; {hint}",
                binary.display()
            )));
        }
    };
    let output = tokio::time::timeout(TOOL_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| ToolError::TimedOut)?
        .map_err(|error| ToolError::Failed(format!("{}: {error}", binary.display())))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolError::Failed(format!(
            "{} exited {}: {}",
            binary.display(),
            output.status.code().unwrap_or(-1),
            stderr.trim().chars().take(500).collect::<String>()
        )));
    }
    if output.stdout.len() > MAX_OUTPUT_BYTES {
        return Err(ToolError::Failed(format!(
            "{} output exceeds {MAX_OUTPUT_BYTES} bytes",
            binary.display()
        )));
    }
    Ok(Some(output.stdout))
}

async fn run_text(binary: &Path, args: &[&Path], hint: &str) -> Result<String, ToolError> {
    let Some(bytes) = run_output(binary, args, hint).await? else {
        return Err(ToolError::Failed(format!(
            "{} is not installed; {hint}",
            binary.display()
        )));
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn zip_entries(binary: &Path, path: &Path) -> Result<Vec<String>, ToolError> {
    let args = [Path::new("-Z1"), path];
    let Some(bytes) = run_output(binary, &args, "unzip (apt install unzip)").await? else {
        return Err(ToolError::Failed(
            "unzip is not installed; apt install unzip".into(),
        ));
    };
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect())
}

async fn zip_read(binary: &Path, path: &Path, entry: &str) -> Result<String, ToolError> {
    let args = [Path::new("-p"), path, Path::new(entry)];
    let Some(bytes) = run_output(binary, &args, "unzip (apt install unzip)").await? else {
        return Err(ToolError::Failed(
            "unzip is not installed; apt install unzip".into(),
        ));
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn extract_zip_text(
    binary: &Path,
    path: &Path,
    prefixes: &[&str],
    strip: bool,
) -> Result<String, ToolError> {
    let entries = zip_entries(binary, path).await?;
    let mut selected = entries
        .into_iter()
        .filter(|entry| prefixes.iter().any(|prefix| entry.starts_with(prefix)))
        .collect::<Vec<_>>();
    selected.sort();
    if selected.is_empty() {
        return Err(ToolError::Failed(
            "document archive has no expected XML entry".into(),
        ));
    }
    let mut out = String::new();
    for entry in selected {
        let text = zip_read(binary, path, &entry).await?;
        let text = if strip { strip_markup(&text) } else { text };
        out.push_str(&format!("--- {entry} ---\n"));
        out.push_str(&text);
        out.push('\n');
    }
    Ok(out)
}

async fn extract_pptx(binary: &Path, path: &Path) -> Result<String, ToolError> {
    let mut slides = zip_entries(binary, path)
        .await?
        .into_iter()
        .filter(|entry| entry.starts_with("ppt/slides/slide") && entry.ends_with(".xml"))
        .collect::<Vec<_>>();
    slides.sort();
    let mut out = String::new();
    for (index, entry) in slides.iter().enumerate() {
        let text = zip_read(binary, path, entry).await?;
        out.push_str(&format!(
            "--- slide {} ---\n{}\n",
            index + 1,
            strip_markup(&text)
        ));
    }
    Ok(out)
}

async fn extract_xlsx(binary: &Path, path: &Path) -> Result<String, ToolError> {
    let shared = match zip_read(binary, path, "xl/sharedStrings.xml").await {
        Ok(value) => extract_tag_values(&value, "t"),
        Err(_) => Vec::new(),
    };
    let mut sheets = zip_entries(binary, path)
        .await?
        .into_iter()
        .filter(|entry| entry.starts_with("xl/worksheets/sheet") && entry.ends_with(".xml"))
        .collect::<Vec<_>>();
    sheets.sort();
    let mut out = String::new();
    for entry in sheets {
        let xml = zip_read(binary, path, &entry).await?;
        out.push_str(&format!("--- {entry} ---\n"));
        out.push_str(&extract_xlsx_sheet(&xml, &shared));
    }
    Ok(out)
}

async fn extract_epub(binary: &Path, path: &Path) -> Result<String, ToolError> {
    let mut entries = zip_entries(binary, path)
        .await?
        .into_iter()
        .filter(|entry| entry.ends_with(".xhtml") || entry.ends_with(".html"))
        .collect::<Vec<_>>();
    entries.sort();
    let mut out = String::new();
    for entry in entries {
        let text = zip_read(binary, path, &entry).await?;
        out.push_str(&format!("--- {entry} ---\n{}\n", strip_markup(&text)));
    }
    Ok(out)
}

fn extract_tag_values(xml: &str, tag: &str) -> Vec<String> {
    let mut values = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let body = &after[open_end + 1..];
        let Some(end) = body.find(&close) else { break };
        values.push(decode_entities(&body[..end]));
        rest = &body[end + close.len()..];
    }
    values
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn inner_tag<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = body.find(&open)?;
    let open_end = body[start..].find('>')? + start;
    let end = body[open_end + 1..].find(&close)? + open_end + 1;
    Some(&body[open_end + 1..end])
}

fn extract_xlsx_sheet(xml: &str, shared: &[String]) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<c") {
        let after = &rest[start + 2..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let tag = &after[..open_end];
        let body = &after[open_end + 1..];
        if after[..=open_end].ends_with("/>") {
            rest = body;
            continue;
        }
        let Some(end) = body.find("</c>") else { break };
        let cell = &body[..end];
        rest = &body[end + 4..];
        let reference = attr(tag, "r").unwrap_or_default();
        let kind = attr(tag, "t").unwrap_or_default();
        let raw = inner_tag(cell, "v")
            .or_else(|| inner_tag(cell, "t"))
            .unwrap_or("");
        let value = if kind == "s" {
            raw.trim()
                .parse::<usize>()
                .ok()
                .and_then(|index| shared.get(index))
                .cloned()
                .unwrap_or_default()
        } else {
            decode_entities(raw)
        };
        if !value.trim().is_empty() {
            out.push_str(&format!("{reference}: {value}\n"));
        }
    }
    out
}

async fn list_zip(binary: &Path, path: &Path) -> Result<String, ToolError> {
    let entries = zip_entries(binary, path).await?;
    Ok(entries
        .into_iter()
        .take(MAX_ARCHIVE_ENTRIES)
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn list_tar(binary: &Path, path: &Path) -> Result<String, ToolError> {
    let args = [Path::new("-tf"), path];
    let Some(bytes) = run_output(binary, &args, "tar").await? else {
        return Err(ToolError::Failed("tar is not installed".into()));
    };
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .take(MAX_ARCHIVE_ENTRIES)
        .collect::<Vec<_>>()
        .join("\n"))
}

const OCR_SCRIPT: &str = r#"
import sys
try:
    from rapidocr_onnxruntime import RapidOCR
except Exception as exc:
    print(f"rapidocr unavailable: {exc}", file=sys.stderr)
    raise SystemExit(3)
engine = RapidOCR()
result, _ = engine(sys.argv[1])
for line in result or []:
    print(line[1])
"#;

async fn ocr_image(
    tesseract: &Path,
    ocr_python: Option<&Path>,
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<String, ToolError> {
    let lang = std::env::var("TESSERACT_LANG").unwrap_or_else(|_| "eng+chi_sim".into());
    let args = [
        path,
        Path::new("stdout"),
        Path::new("-l"),
        Path::new(lang.as_str()),
    ];
    let mut tesseract_error = None;
    match run_output(tesseract, &args, "tesseract (apt install tesseract-ocr)").await {
        Ok(Some(bytes)) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Ok(None) => {}
        Err(first) => {
            let args = [path, Path::new("stdout"), Path::new("-l"), Path::new("eng")];
            match run_output(tesseract, &args, "tesseract (apt install tesseract-ocr)").await {
                Ok(Some(bytes)) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
                Ok(None) => {}
                Err(_) => tesseract_error = Some(first),
            }
        }
    }
    if cancellation.is_cancelled() {
        return Err(ToolError::Cancelled);
    }
    if let Some(python) = ocr_python {
        let args = [
            Path::new("-I"),
            Path::new("-c"),
            Path::new(OCR_SCRIPT),
            path,
        ];
        match run_output(
            python,
            &args,
            "OCR_PYTHON must point at a Python with rapidocr_onnxruntime",
        )
        .await
        {
            Ok(Some(bytes)) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Ok(None) => {}
            Err(error) => {
                if tesseract_error.is_none() {
                    tesseract_error = Some(error);
                }
            }
        }
    }
    Err(tesseract_error.unwrap_or_else(|| {
        ToolError::Failed(
            "image OCR requires tesseract or OCR_PYTHON with rapidocr_onnxruntime; install tesseract-ocr or set OCR_PYTHON to an isolated venv"
                .into(),
        )
    }))
}

fn strip_markup(input: &str) -> String {
    let input = remove_block(input, "script");
    let input = remove_block(&input, "style");
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '<' {
            out.push(ch);
            continue;
        }
        let mut tag = String::new();
        for (_, c) in chars.by_ref() {
            if c == '>' {
                break;
            }
            tag.push(c);
        }
        let name = tag
            .trim()
            .trim_start_matches('/')
            .trim_start_matches('!')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/')
            .to_ascii_lowercase();
        if is_block_tag(&name) || name == "br" || name == "tab" {
            out.push('\n');
        }
    }
    collapse_blank_lines(&decode_entities(&out))
}

fn remove_block(input: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::new();
    let mut rest = input;
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let Some(end_rel) = lower[start..].find(&close) else {
            break;
        };
        rest = &rest[start + end_rel + close.len()..];
    }
    out
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "li"
            | "tr"
            | "hr"
            | "section"
            | "article"
            | "title"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "w:p"
            | "a:p"
            | "text:p"
            | "row"
            | "w:tr"
            | "a:tr"
    )
}

fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::new();
    let mut blanks = 0usize;
    for line in input.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let mut entity = String::new();
        let mut found = false;
        for (_, c) in chars.by_ref() {
            if c == ';' {
                found = true;
                break;
            }
            if entity.len() > 12 {
                break;
            }
            entity.push(c);
        }
        if !found {
            out.push('&');
            out.push_str(&entity);
            continue;
        }
        let decoded = match entity.as_str() {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            value if value.starts_with("#x") || value.starts_with("#X") => {
                u32::from_str_radix(&value[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            value if value.starts_with('#') => {
                value[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        if let Some(value) = decoded {
            out.push(value);
        } else {
            out.push('&');
            out.push_str(&entity);
            out.push(';');
        }
    }
    out
}

fn strip_rtf(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    let mut depth = 0i32;
    let mut skip_depth: Option<i32> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                if skip_depth == Some(depth) {
                    skip_depth = None;
                }
                depth = (depth - 1).max(0);
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    break;
                }
                let c = bytes[i];
                if c == b'\'' {
                    if i + 2 < bytes.len() {
                        if let Ok(value) = u8::from_str_radix(&input[i + 1..i + 3], 16)
                            && skip_depth.is_none()
                        {
                            out.push(value as char);
                        }
                        i += 3;
                    } else {
                        i += 1;
                    }
                } else if matches!(c, b'\\' | b'{' | b'}') {
                    if skip_depth.is_none() {
                        out.push(c as char);
                    }
                    i += 1;
                } else if c.is_ascii_alphabetic() {
                    let start = i;
                    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word = &input[start..i];
                    let mut number = 0i32;
                    let mut negative = false;
                    if i < bytes.len() && bytes[i] == b'-' {
                        negative = true;
                        i += 1;
                    }
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        number = number
                            .saturating_mul(10)
                            .saturating_add((bytes[i] - b'0') as i32);
                        i += 1;
                    }
                    if negative {
                        number = -number;
                    }
                    // A space after a control word is a delimiter, not text.
                    if i < bytes.len() && bytes[i] == b' ' {
                        i += 1;
                    }
                    match word {
                        "par" | "line" => {
                            if skip_depth.is_none() {
                                out.push('\n');
                            }
                        }
                        "tab" => {
                            if skip_depth.is_none() {
                                out.push('\t');
                            }
                        }
                        "u" => {
                            if skip_depth.is_none() {
                                let value = if number < 0 {
                                    (number + 65536) as u32
                                } else {
                                    number as u32
                                };
                                if let Some(ch) = char::from_u32(value) {
                                    out.push(ch);
                                }
                            }
                            if i < bytes.len() && bytes[i] == b'?' {
                                i += 1;
                            }
                        }
                        "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" => {
                            skip_depth = Some(depth);
                        }
                        _ => {}
                    }
                } else {
                    i += 1;
                }
            }
            value => {
                if skip_depth.is_none() {
                    out.push(value as char);
                }
                i += 1;
            }
        }
    }
    collapse_blank_lines(&out)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn bins() -> DocumentBinaries {
        DocumentBinaries {
            pdf: PathBuf::from("pdftotext"),
            unzip: PathBuf::from("unzip"),
            tesseract: PathBuf::from("tesseract"),
            ocr_python: None,
            catdoc: PathBuf::from("catdoc"),
            xls2csv: PathBuf::from("xls2csv"),
            catppt: PathBuf::from("catppt"),
            sqlite3: PathBuf::from("sqlite3"),
            tar: PathBuf::from("tar"),
        }
    }

    fn permissions(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![root.to_path_buf()],
            allow_network: false,
            allow_write: false,
        }
    }

    fn write_zip(dir: &Path, name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let source = dir.join(format!("{name}-src"));
        for (entry, body) in entries {
            let path = source.join(entry);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let archive = dir.join(name);
        let status = std::process::Command::new("zip")
            .arg("-q")
            .arg("-r")
            .arg(&archive)
            .arg(".")
            .current_dir(&source)
            .status()
            .unwrap();
        assert!(status.success());
        archive
    }

    #[tokio::test]
    async fn reads_text_html_xml_and_rtf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), "hello markdown").unwrap();
        std::fs::write(
            dir.path().join("page.html"),
            "<html><head><style>x{}</style></head><body><h1>Title</h1><script>bad()</script><p>Hello <b>world</b></p></body></html>",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("data.xml"),
            "<root><item>value</item></root>",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("note.rtf"),
            "{\\rtf1\\ansi Hello\\par World}",
        )
        .unwrap();
        let tool = ReadDocumentTool::new(bins());
        for (name, expected) in [
            ("note.md", "hello markdown"),
            ("page.html", "Title"),
            ("data.xml", "<root>"),
            ("note.rtf", "Hello\nWorld"),
        ] {
            let outcome = tool
                .execute(
                    json!({"path": name}),
                    &permissions(dir.path()),
                    &CancellationToken::new(),
                )
                .await
                .unwrap();
            assert!(
                outcome.content.contains(expected),
                "{name}: {}",
                outcome.content
            );
        }
    }

    #[tokio::test]
    async fn reads_docx_pptx_and_xlsx() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(
            dir.path(),
            "sample.docx",
            &[(
                "word/document.xml",
                "<w:document><w:body><w:p><w:r><w:t>Hello DOCX</w:t></w:r></w:p></w:body></w:document>",
            )],
        );
        let pptx = write_zip(
            dir.path(),
            "sample.pptx",
            &[(
                "ppt/slides/slide1.xml",
                "<p:sld><a:t>Hello PPTX</a:t></p:sld>",
            )],
        );
        let xlsx = write_zip(
            dir.path(),
            "sample.xlsx",
            &[
                (
                    "xl/sharedStrings.xml",
                    "<sst><si><t>Hello XLSX</t></si></sst>",
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    "<worksheet><sheetData><row><c r=\"A1\" t=\"s\"><v>0</v></c></row></sheetData></worksheet>",
                ),
            ],
        );
        let tool = ReadDocumentTool::new(bins());
        for (file, expected) in [
            (path, "Hello DOCX"),
            (pptx, "Hello PPTX"),
            (xlsx, "A1: Hello XLSX"),
        ] {
            let outcome = tool
                .execute(
                    json!({"path": file.display().to_string()}),
                    &permissions(dir.path()),
                    &CancellationToken::new(),
                )
                .await
                .unwrap();
            assert!(outcome.content.contains(expected), "{}", outcome.content);
        }
    }

    #[tokio::test]
    async fn lists_archives_and_rejects_escape_paths() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_zip(dir.path(), "bundle.zip", &[("notes/readme.txt", "hello")]);
        let tool = ReadDocumentTool::new(bins());
        let outcome = tool
            .execute(
                json!({"path": archive.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("notes/readme.txt"));

        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("x.txt");
        std::fs::write(&outside_file, "x").unwrap();
        let error = tool
            .execute(
                json!({"path": outside_file.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));

        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&outside_file, &link).unwrap();
        let error = tool
            .execute(
                json!({"path": link.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn image_without_ocr_backend_fails_with_a_hint() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("pic.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\n").unwrap();
        let mut binaries = bins();
        binaries.tesseract = dir.path().join("no-tesseract");
        let tool = ReadDocumentTool::new(binaries);
        let error = tool
            .execute(
                json!({"path": image.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error}").contains("tesseract"), "{error}");
    }

    #[tokio::test]
    async fn real_ocr_backend_reads_fixture_when_available() {
        let bins = DocumentBinaries::from_env();
        if bins.ocr_python.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("ocr.png");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ocr.png"),
            &image,
        )
        .unwrap();
        let tool = ReadDocumentTool::new(bins);
        let outcome = tool
            .execute(
                json!({"path": image.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        let text = outcome.content.to_ascii_uppercase();
        assert!(text.contains("HELLO"), "{}", outcome.content);
        assert!(text.contains("12345"), "{}", outcome.content);
    }

    #[tokio::test]
    async fn image_uses_ocr_python_when_tesseract_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("pic.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\n").unwrap();
        let python = dir.path().join("ocr-python");
        std::fs::write(&python, "#!/bin/sh\nprintf 'OCR TEXT\\n'\n").unwrap();
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut binaries = bins();
        binaries.tesseract = dir.path().join("no-tesseract");
        binaries.ocr_python = Some(python);
        let tool = ReadDocumentTool::new(binaries);
        let outcome = tool
            .execute(
                json!({"path": image.display().to_string()}),
                &permissions(dir.path()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("OCR TEXT"), "{}", outcome.content);
    }

    #[test]
    fn markup_and_entities_are_reduced_to_text() {
        let text = strip_markup(
            "<html><script>bad()</script><h1>Title &amp; more</h1><p>Hello&nbsp;world</p></html>",
        );
        assert!(text.contains("Title & more"));
        assert!(text.contains("Hello world"));
        assert!(!text.contains("bad()"));
    }
}
