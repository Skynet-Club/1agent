pub fn encode_segment(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        let character = *byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~') {
            output.push(character);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    if output.is_empty() {
        "%00".into()
    } else {
        output
    }
}

pub fn session_id(id: &str) -> String {
    format!("urn:1agent:session:{}", encode_segment(id))
}

pub fn run_id(id: &str) -> String {
    format!("urn:1agent:run:{}", encode_segment(id))
}

pub fn task_id(run: &str) -> String {
    format!("urn:1agent:task:{}:main", encode_segment(run))
}

pub fn tool_call_id(run: &str, call: &str) -> String {
    format!("urn:1agent:tool-call:{}:{}", encode_segment(run), encode_segment(call))
}

pub fn observation_id(run: &str, call: &str) -> String {
    format!("urn:1agent:observation:{}:{}", encode_segment(run), encode_segment(call))
}

pub fn answer_id(run: &str) -> String {
    format!("urn:1agent:answer:{}", encode_segment(run))
}

pub fn claim_id(run: &str, claim: &str) -> String {
    format!("urn:1agent:claim:{}:{}", encode_segment(run), encode_segment(claim))
}

pub fn evidence_id(run: &str, seq: u64, index: usize) -> String {
    format!(
        "urn:1agent:evidence:{}:{}:{}",
        encode_segment(run),
        seq,
        index
    )
}

pub fn artifact_id(sha256: &str) -> String {
    format!("urn:1agent:artifact:sha256:{}", encode_segment(sha256))
}

pub fn goal_id(id: &str) -> String {
    format!("urn:1agent:goal:{}", encode_segment(id))
}

pub fn commitment_id(id: &str) -> String {
    format!("urn:1agent:commitment:{}", encode_segment(id))
}
