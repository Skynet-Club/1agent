//! Live extraction test for the system `pdftotext`; ignored by default because
//! it needs poppler-utils and ghostscript.

use std::process::Command;

use first_principles_agent::{AgentTool, PdfReadTool, PermissionSet};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore]
async fn real_pdftotext_extracts_text_from_a_generated_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let postscript = dir.path().join("source.ps");
    std::fs::write(
        &postscript,
        "%!PS\n/Helvetica findfont 18 scalefont setfont\n72 720 moveto\n(Hello 1A PDF) show\nshowpage\n",
    )
    .unwrap();
    let pdf = dir.path().join("source.pdf");
    // Ghostscript's scratch files fail under the harness temp directory on
    // this host; /tmp is the only location it opens successfully.
    let output = Command::new("ps2pdf")
        .env("TMPDIR", "/tmp")
        .arg(&postscript)
        .arg(&pdf)
        .output()
        .expect("ps2pdf is required for the live PDF test");
    assert!(
        output.status.success(),
        "ps2pdf stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tool = PdfReadTool::new("pdftotext");
    let outcome = tool
        .execute(
            serde_json::json!({"path": pdf.display().to_string()}),
            &PermissionSet {
                read_roots: vec![dir.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        outcome.content.contains("Hello 1A PDF"),
        "content: {}",
        outcome.content
    );
}
